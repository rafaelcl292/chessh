use std::sync::Arc;

use ratatui::backend::Backend;
use ratatui::Terminal;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::server::SessionManager;
use crate::ssh::session::{GameEvent, SessionId};
use crate::ui::{App, AppAction, AppView, InputDecoder, InputEvent, SshBackend};

pub struct SessionRunner {
    session_id: SessionId,
    session_manager: Arc<RwLock<SessionManager>>,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
    game_event_rx: mpsc::UnboundedReceiver<GameEvent>,
    width: u16,
    height: u16,
}

impl SessionRunner {
    pub fn new(
        session_id: SessionId,
        session_manager: Arc<RwLock<SessionManager>>,
        input_rx: mpsc::Receiver<Vec<u8>>,
        output_tx: mpsc::Sender<Vec<u8>>,
        game_event_rx: mpsc::UnboundedReceiver<GameEvent>,
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

        let _ = self
            .output_tx
            .send(b"\x1b[?1000h\x1b[?1006h".to_vec())
            .await;
        let mut decoder = InputDecoder::default();
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

            app.set_area(ratatui::layout::Rect::new(0, 0, self.width, self.height));
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
                            for event in decoder.feed(&data) {
                                let action = app.handle_input(event);
                                self.handle_action(&mut app, action, &mut current_game_id).await;
                                if let InputEvent::Resize(w, h) = event {
                                    self.width = w;
                                    self.height = h;
                                    app.set_area(ratatui::layout::Rect::new(0, 0, w, h));
                                    terminal.backend_mut().resize(w, h);
                                }
                                if app.should_quit() { break; }
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
                    if let Some(event) = decoder.flush_escape() {
                        let action = app.handle_input(event);
                        self.handle_action(&mut app, action, &mut current_game_id).await;
                    }
                    let manager = self.session_manager.read().await;
                    app.set_online_count(manager.session_count());
                    app.set_queue_size(manager.queue_size());
                }
            }
        }

        self.session_manager
            .write()
            .await
            .remove_session(self.session_id);

        let _ = terminal.clear();
        let _ = terminal.show_cursor();
        let _ = terminal.backend_mut().flush();

        let goodbye = "\x1b[?1000l\x1b[?1006l\x1b[?25h\x1b[0m\x1b[2J\x1b[H\r\nGoodbye!\r\n";
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
                let mut manager = self.session_manager.write().await;
                manager.join_queue(self.session_id);
                app.join_queue();
                manager.try_match();
            }
            AppAction::LeaveQueue => {
                let mut manager = self.session_manager.write().await;
                // A match may already have been assigned while Esc was in flight.
                if manager.get_player_game_id(self.session_id).is_none() {
                    manager.leave_queue(self.session_id);
                    app.leave_queue();
                }
            }
            AppAction::Quit => app.set_should_quit(true),
            AppAction::Resign => {
                if let Some(id) = *current_game_id {
                    if let Err(error) = self
                        .session_manager
                        .write()
                        .await
                        .resign_game(self.session_id, id)
                    {
                        app.set_status(Some(error));
                    }
                } else if let Some(game) = app.game_mut() {
                    game.resign(game.turn());
                    app.finish_game("Resignation".to_string());
                }
            }
            AppAction::OfferDraw => {
                if let Some(id) = *current_game_id {
                    match self
                        .session_manager
                        .write()
                        .await
                        .offer_draw(self.session_id, id)
                    {
                        Ok(()) => app.set_status(Some("Draw offered".to_string())),
                        Err(error) => app.set_status(Some(error)),
                    }
                }
            }
            AppAction::SubmitMove(from, to) => {
                self.submit_move(app, *current_game_id, &format!("{from}{to}"))
                    .await;
            }
            AppAction::SubmitMoveText(text) => {
                self.submit_move(app, *current_game_id, &text).await;
            }
            AppAction::ReturnToLobby => *current_game_id = None,
            AppAction::None => {}
        }
        // Solo moves can be played directly by the UI as well as by an action.
        if !app.is_multiplayer() && app.view() == AppView::Game {
            if let Some(game) = app.game() {
                if game.is_game_over() {
                    let reason = if game.is_checkmate() {
                        "Checkmate"
                    } else if game.is_stalemate() {
                        "Stalemate"
                    } else {
                        "Insufficient material"
                    };
                    app.finish_game(reason.to_string());
                }
            }
        }
    }

    async fn submit_move(&self, app: &mut App, game_id: Option<u64>, text: &str) {
        let result = if let Some(id) = game_id {
            self.session_manager
                .write()
                .await
                .submit_move(self.session_id, id, text)
        } else if let Some(game) = app.game_mut() {
            game.play_uci(text)
        } else {
            return;
        };
        if let Err(error) = result {
            app.set_status(Some(error));
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
                if *current_game_id != Some(game_id) && app.view() == AppView::InQueue {
                    *current_game_id = Some(game_id);
                    app.start_game(opponent_name, !is_white);
                }
            }
            GameEvent::StateUpdated {
                game_id,
                game,
                finished_reason,
            } => {
                if *current_game_id != Some(game_id) {
                    return;
                }
                app.update_game(game);
                if let Some(reason) = finished_reason {
                    app.finish_game(reason);
                    *current_game_id = None;
                }
            }
            GameEvent::DrawOffered { game_id } => {
                if *current_game_id == Some(game_id) {
                    app.set_status(Some("Opponent offers a draw. /draw to accept".to_string()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::Game;
    use crate::ssh::session::TerminalSize;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn runner() -> SessionRunner {
        let (_, input) = mpsc::channel(8);
        let (output, _) = mpsc::channel(8);
        let (_, events) = mpsc::unbounded_channel();
        SessionRunner::new(
            SessionId::new(),
            Arc::new(RwLock::new(SessionManager::new())),
            input,
            output,
            events,
            80,
            24,
        )
    }

    #[tokio::test]
    async fn final_snapshot_is_applied_before_game_over_and_old_events_are_ignored() {
        let mut runner = runner();
        let mut app = App::new("player".into());
        app.join_queue();
        let mut current = None;
        runner
            .handle_game_event(
                &mut app,
                GameEvent::MatchFound {
                    game_id: 1,
                    opponent_name: "opponent".into(),
                    is_white: true,
                },
                &mut current,
            )
            .await;
        let mut game = Game::new();
        for mv in ["f3", "e5", "g4", "Qh4#"] {
            game.play_san(mv).unwrap();
        }
        let fen = game.fen();
        runner
            .handle_game_event(
                &mut app,
                GameEvent::StateUpdated {
                    game_id: 1,
                    game,
                    finished_reason: Some("Checkmate".into()),
                },
                &mut current,
            )
            .await;
        assert_eq!(app.view(), AppView::GameOver);
        assert_eq!(app.game().unwrap().fen(), fen);
        assert_eq!(current, None);
        app.return_to_lobby();
        app.join_queue();
        runner
            .handle_game_event(
                &mut app,
                GameEvent::MatchFound {
                    game_id: 2,
                    opponent_name: "next".into(),
                    is_white: false,
                },
                &mut current,
            )
            .await;
        runner
            .handle_game_event(
                &mut app,
                GameEvent::StateUpdated {
                    game_id: 1,
                    game: Game::new(),
                    finished_reason: Some("old event".into()),
                },
                &mut current,
            )
            .await;
        assert_eq!(app.view(), AppView::Game);
        assert_eq!(current, Some(2));
    }

    #[tokio::test]
    async fn solo_typed_mate_enters_finished_state() {
        let mut runner = runner();
        let mut app = App::new("solo".into());
        app.start_solo_game();
        let mut current = None;
        for text in ["f3", "e5", "g4", "Qh4"] {
            for key in text.chars().map(KeyCode::Char).chain([KeyCode::Enter]) {
                let action = app.handle_input(InputEvent::Key(key, KeyModifiers::NONE));
                runner.handle_action(&mut app, action, &mut current).await;
            }
        }
        assert_eq!(app.view(), AppView::GameOver);
        assert!(app.game().unwrap().is_checkmate());
    }

    #[tokio::test]
    async fn cancel_after_match_assignment_does_not_abandon_game() {
        let mut runner = runner();
        let mut app = App::new("player".into());
        app.join_queue();
        {
            let mut manager = runner.session_manager.write().await;
            for id in [runner.session_id, SessionId::new()] {
                manager.add_session(id, TerminalSize::default());
                manager.join_queue(id);
            }
            manager.try_match().unwrap();
        }
        runner
            .handle_action(&mut app, AppAction::LeaveQueue, &mut None)
            .await;
        assert_eq!(app.view(), AppView::InQueue);
        assert!(runner
            .session_manager
            .read()
            .await
            .get_player_game_id(runner.session_id)
            .is_some());
    }
}
