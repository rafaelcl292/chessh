use std::sync::Arc;

use ratatui::backend::Backend;
use ratatui::Terminal;
use shakmaty::Square;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::server::SessionManager;
use crate::ssh::session::{GameEvent, SessionId};
use crate::ui::{parse_input, App, AppAction, AppView, GameOverReason, InputEvent, SshBackend};

pub struct SessionRunner {
    session_id: SessionId,
    session_manager: Arc<RwLock<SessionManager>>,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
    game_event_rx: mpsc::Receiver<GameEvent>,
    width: u16,
    height: u16,
}

impl SessionRunner {
    pub fn new(
        session_id: SessionId,
        session_manager: Arc<RwLock<SessionManager>>,
        input_rx: mpsc::Receiver<Vec<u8>>,
        output_tx: mpsc::Sender<Vec<u8>>,
        game_event_rx: mpsc::Receiver<GameEvent>,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            session_id,
            session_manager,
            input_rx,
            output_tx,
            game_event_rx,
            width,
            height,
        }
    }

    pub async fn run(mut self, username: String) {
        info!("Starting session runner for {}", self.session_id);

        let backend = SshBackend::new(self.output_tx.clone(), self.width, self.height);
        let mut terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to create terminal: {}", e);
                return;
            }
        };

        let _ = terminal.clear();

        let mut app = App::new(username);
        let mut current_game_id: Option<u64> = None;

        {
            let manager = self.session_manager.read().await;
            app.set_online_count(manager.session_count());
            app.set_queue_size(manager.queue_size());
        }

        loop {
            if app.should_quit() {
                break;
            }

            if let Err(e) = terminal.draw(|frame| {
                app.draw(frame);
            }) {
                error!("Failed to draw: {}", e);
                break;
            }

            tokio::select! {
                input = self.input_rx.recv() => {
                    match input {
                        Some(data) => {
                            let event = parse_input(&data);
                            let action = app.handle_input(event);

                            self.handle_action(&mut app, action, &mut current_game_id).await;

                            if let InputEvent::Resize(w, h) = event {
                                terminal.backend_mut().resize(w, h);
                            }
                        }
                        None => {
                            debug!("Input channel closed");
                            break;
                        }
                    }
                }

                game_event = self.game_event_rx.recv() => {
                    if let Some(event) = game_event {
                        self.handle_game_event(&mut app, event, &mut current_game_id).await;
                    }
                }

                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    if app.view() == AppView::InQueue {
                        self.check_for_match(&mut app, &mut current_game_id).await;
                    }

                    let manager = self.session_manager.read().await;
                    app.set_online_count(manager.session_count());
                    app.set_queue_size(manager.queue_size());
                }
            }
        }

        if let Some(game_id) = current_game_id {
            let mut manager = self.session_manager.write().await;
            manager.end_game(game_id);
        }

        let _ = terminal.clear();
        let _ = terminal.show_cursor();
        let _ = terminal.backend_mut().flush();

        let goodbye = "\x1b[?25h\x1b[0m\x1b[2J\x1b[H\r\nGoodbye!\r\n";
        let _ = self.output_tx.send(goodbye.as_bytes().to_vec()).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        info!("Session runner ended for {}", self.session_id);
    }

    async fn handle_action(
        &mut self,
        app: &mut App,
        action: AppAction,
        current_game_id: &mut Option<u64>,
    ) {
        match action {
            AppAction::JoinQueue => {
                app.join_queue();
                let mut manager = self.session_manager.write().await;
                manager.join_queue(self.session_id);
                app.set_queue_size(manager.queue_size());

                if let Some(game_id) = manager.try_match() {
                    drop(manager);
                    self.notify_matched_players(game_id).await;
                }
            }
            AppAction::LeaveQueue => {
                app.leave_queue();
                let mut manager = self.session_manager.write().await;
                manager.leave_queue(self.session_id);
                app.set_queue_size(manager.queue_size());
            }
            AppAction::Quit => {
                app.set_should_quit(true);
            }
            AppAction::Resign => {
                if let Some(game_id) = *current_game_id {
                    self.handle_resign(app, game_id, current_game_id).await;
                }
            }
            AppAction::OfferDraw => {
                if let Some(game_id) = *current_game_id {
                    self.handle_draw_offer(app, game_id, current_game_id).await;
                }
            }
            AppAction::SubmitMove(from, to) => {
                if let Some(game_id) = *current_game_id {
                    let uci = format!("{}{}", from, to);
                    self.handle_move_text(app, game_id, &uci, current_game_id)
                        .await;
                } else {
                    let uci = format!("{}{}", from, to);
                    if let Some(game) = app.game_mut() {
                        match game.play_uci(&uci) {
                            Ok(_) => {
                                app.set_status(None);
                                self.check_solo_game_over(app);
                            }
                            Err(e) => app.set_status(Some(format!("Invalid move: {}", e))),
                        }
                    }
                }
            }
            AppAction::SubmitMoveText(text) => {
                if let Some(game_id) = *current_game_id {
                    self.handle_move_text(app, game_id, &text, current_game_id)
                        .await;
                }
            }
            AppAction::ReturnToLobby => {
                *current_game_id = None;
            }
            AppAction::None => {}
        }
    }

    async fn handle_game_event(
        &mut self,
        app: &mut App,
        event: GameEvent,
        current_game_id: &mut Option<u64>,
    ) {
        match event {
            GameEvent::MatchFound {
                game_id,
                opponent_name,
                is_white,
            } => {
                *current_game_id = Some(game_id);
                app.start_game(opponent_name, !is_white);
            }
            GameEvent::MovePlayed {
                from,
                to,
                promotion,
            } => {
                let uci = match promotion {
                    Some(p) => format!("{}{}{}", from, to, p),
                    None => format!("{}{}", from, to),
                };
                if let Some(game) = app.game_mut() {
                    let _ = game.play_uci(&uci);
                    self.check_game_over(app, current_game_id);
                }
            }
            GameEvent::DrawOffered => {
                app.set_status(Some("Opponent offers a draw. /draw to accept".to_string()));
            }
            GameEvent::DrawAccepted => {
                if let Some(game) = app.game_mut() {
                    game.set_result(crate::chess::GameResult::Draw);
                }
                app.show_game_over(GameOverReason::Draw("Draw by agreement".to_string()));
                *current_game_id = None;
            }
            GameEvent::DrawDeclined => {
                app.set_status(Some("Draw declined".to_string()));
            }
            GameEvent::OpponentResigned => {
                app.show_game_over(GameOverReason::YouWin("Opponent resigned".to_string()));
                *current_game_id = None;
            }
            GameEvent::OpponentDisconnected => {
                app.show_game_over(GameOverReason::YouWin("Opponent disconnected".to_string()));
                *current_game_id = None;
            }
            GameEvent::GameEnded { reason } => {
                app.show_game_over(GameOverReason::Draw(reason));
                *current_game_id = None;
            }
        }
    }

    fn check_game_over(&self, app: &mut App, current_game_id: &mut Option<u64>) {
        if let Some(game) = app.game() {
            if game.is_checkmate() {
                let loser_turn = game.turn();
                let reason = if (loser_turn == shakmaty::Color::White && app.is_my_turn())
                    || (loser_turn == shakmaty::Color::Black && !app.is_my_turn())
                {
                    GameOverReason::YouLose("Checkmate".to_string())
                } else {
                    GameOverReason::YouWin("Checkmate".to_string())
                };
                app.show_game_over(reason);
                *current_game_id = None;
            } else if game.is_stalemate() {
                app.show_game_over(GameOverReason::Draw("Stalemate".to_string()));
                *current_game_id = None;
            }
        }
    }

    fn check_solo_game_over(&self, app: &mut App) {
        if let Some(game) = app.game() {
            if game.is_checkmate() {
                let loser_turn = game.turn();
                let reason = if loser_turn == shakmaty::Color::White {
                    GameOverReason::YouLose("Checkmate - Black wins".to_string())
                } else {
                    GameOverReason::YouWin("Checkmate - White wins".to_string())
                };
                app.show_game_over(reason);
            } else if game.is_stalemate() {
                app.show_game_over(GameOverReason::Draw("Stalemate".to_string()));
            }
        }
    }

    async fn check_for_match(&mut self, app: &mut App, current_game_id: &mut Option<u64>) {
        let manager = self.session_manager.read().await;
        if let Some(game_id) = manager.get_player_game_id(self.session_id) {
            if let Some(info) = manager.get_match_info(game_id) {
                let is_white = info.white_id == self.session_id;
                let opponent_name = if is_white {
                    info.black_name.clone()
                } else {
                    info.white_name.clone()
                };
                *current_game_id = Some(game_id);
                app.start_game(opponent_name, !is_white);
            }
        }
    }

    async fn notify_matched_players(&self, game_id: u64) {
        let manager = self.session_manager.read().await;
        if let Some(info) = manager.get_match_info(game_id) {
            if let Some(tx) = manager.get_game_event_tx(info.white_id) {
                let _ = tx
                    .send(GameEvent::MatchFound {
                        game_id,
                        opponent_name: info.black_name.clone(),
                        is_white: true,
                    })
                    .await;
            }
            if let Some(tx) = manager.get_game_event_tx(info.black_id) {
                let _ = tx
                    .send(GameEvent::MatchFound {
                        game_id,
                        opponent_name: info.white_name.clone(),
                        is_white: false,
                    })
                    .await;
            }
        }
    }

    async fn handle_move_text(
        &self,
        app: &mut App,
        game_id: u64,
        text: &str,
        current_game_id: &mut Option<u64>,
    ) {
        let mut manager = self.session_manager.write().await;

        if let Some(game_session) = manager.get_game_mut(game_id) {
            match game_session.play_move_text(self.session_id, text) {
                Ok(uci) => {
                    if let Some(game) = app.game_mut() {
                        let _ = game.play_uci(&uci);
                    }
                    app.set_status(None);

                    let opponent_id = game_session.get_opponent(self.session_id);
                    drop(manager);

                    if let Some(opp_id) = opponent_id {
                        if uci.len() >= 4 {
                            if let (Ok(from), Ok(to)) =
                                (uci[0..2].parse::<Square>(), uci[2..4].parse::<Square>())
                            {
                                let promotion = uci.chars().nth(4);
                                let manager = self.session_manager.read().await;
                                if let Some(tx) = manager.get_game_event_tx(opp_id) {
                                    let _ = tx
                                        .send(GameEvent::MovePlayed {
                                            from,
                                            to,
                                            promotion,
                                        })
                                        .await;
                                }
                            }
                        }
                    }

                    self.check_game_over(app, current_game_id);
                }
                Err(e) => {
                    app.set_status(Some(e.to_string()));
                }
            }
        }
    }

    async fn handle_resign(&self, app: &mut App, game_id: u64, current_game_id: &mut Option<u64>) {
        let mut manager = self.session_manager.write().await;

        if let Some(game_session) = manager.get_game_mut(game_id) {
            if game_session.resign(self.session_id).is_ok() {
                let opponent_id = game_session.get_opponent(self.session_id);
                drop(manager);

                if let Some(opp_id) = opponent_id {
                    let manager = self.session_manager.read().await;
                    if let Some(tx) = manager.get_game_event_tx(opp_id) {
                        let _ = tx.send(GameEvent::OpponentResigned).await;
                    }
                }

                app.show_game_over(GameOverReason::YouLose("You resigned".to_string()));
                *current_game_id = None;
            }
        }
    }

    async fn handle_draw_offer(
        &self,
        app: &mut App,
        game_id: u64,
        current_game_id: &mut Option<u64>,
    ) {
        let mut manager = self.session_manager.write().await;

        if let Some(game_session) = manager.get_game_mut(game_id) {
            let had_pending_offer = game_session.has_pending_draw_offer_for(self.session_id);

            match game_session.offer_draw(self.session_id) {
                Ok(_) => {
                    if had_pending_offer {
                        if let Some(game) = app.game_mut() {
                            game.set_result(crate::chess::GameResult::Draw);
                        }

                        let opponent_id = game_session.get_opponent(self.session_id);
                        drop(manager);

                        if let Some(opp_id) = opponent_id {
                            let manager = self.session_manager.read().await;
                            if let Some(tx) = manager.get_game_event_tx(opp_id) {
                                let _ = tx.send(GameEvent::DrawAccepted).await;
                            }
                        }

                        app.show_game_over(GameOverReason::Draw("Draw by agreement".to_string()));
                        *current_game_id = None;
                    } else {
                        app.set_status(Some("Draw offered".to_string()));

                        let opponent_id = game_session.get_opponent(self.session_id);
                        drop(manager);

                        if let Some(opp_id) = opponent_id {
                            let manager = self.session_manager.read().await;
                            if let Some(tx) = manager.get_game_event_tx(opp_id) {
                                let _ = tx.send(GameEvent::DrawOffered).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    app.set_status(Some(format!("Error: {}", e)));
                }
            }
        }
    }
}
