use std::collections::{HashMap, VecDeque};

use rand::RngExt;
use tokio::sync::mpsc;

use crate::chess::GameResult;
use crate::storage::{GameHistory, GameRecord};

use crate::ssh::session::{GameEvent, SessionId, TerminalSize};

use super::GameSession;

pub struct MatchInfo {
    pub game_id: u64,
    pub white_id: SessionId,
    pub black_id: SessionId,
    pub white_name: String,
    pub black_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    InQueue,
    Playing(u64),
    Spectating,
}

pub struct PlayerSession {
    pub id: SessionId,
    pub username: Option<String>,
    pub state: PlayerState,
    pub terminal_size: TerminalSize,
    pub game_event_tx: Option<mpsc::UnboundedSender<GameEvent>>,
}

pub struct SessionManager {
    sessions: HashMap<SessionId, PlayerSession>,
    queue: VecDeque<SessionId>,
    games: HashMap<u64, GameSession>,
    next_game_id: u64,
    history: GameHistory,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::with_history(GameHistory::new())
    }

    pub fn with_history(history: GameHistory) -> Self {
        Self {
            sessions: HashMap::new(),
            queue: VecDeque::new(),
            games: HashMap::new(),
            next_game_id: history.next_id(),
            history,
        }
    }

    pub fn add_session(&mut self, id: SessionId, terminal_size: TerminalSize) {
        let session = PlayerSession {
            id,
            username: None,
            state: PlayerState::Idle,
            terminal_size,
            game_event_tx: None,
        };
        self.sessions.insert(id, session);
        tracing::info!("Session {} connected. Total: {}", id, self.sessions.len());
    }

    pub fn register_game_event_channel(
        &mut self,
        id: SessionId,
        tx: mpsc::UnboundedSender<GameEvent>,
    ) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.game_event_tx = Some(tx);
        }
    }

    pub fn remove_session(&mut self, id: SessionId) -> Option<u64> {
        self.queue.retain(|&qid| qid != id);

        let game_id = self.get_player_game_id(id);
        if let Some(gid) = game_id {
            if let Some(game) = self.games.get_mut(&gid) {
                let _ = game.resign(id);
            }
            self.finish_game(gid, "Opponent disconnected");
        }

        self.sessions.remove(&id);
        tracing::info!(
            "Session {} disconnected. Total: {}",
            id,
            self.sessions.len()
        );
        game_id
    }

    pub fn get_session(&self, id: SessionId) -> Option<&PlayerSession> {
        self.sessions.get(&id)
    }

    pub fn get_session_mut(&mut self, id: SessionId) -> Option<&mut PlayerSession> {
        self.sessions.get_mut(&id)
    }

    pub fn set_username(&mut self, id: SessionId, username: String) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.username = Some(username);
        }
    }

    pub fn set_state(&mut self, id: SessionId, state: PlayerState) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.state = state;
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }

    pub fn join_queue(&mut self, id: SessionId) {
        if self
            .sessions
            .get(&id)
            .is_some_and(|s| s.state == PlayerState::Idle)
            && !self.queue.contains(&id)
        {
            self.queue.push_back(id);
            if let Some(session) = self.sessions.get_mut(&id) {
                session.state = PlayerState::InQueue;
            }
            tracing::debug!("Added {} to queue. Queue size: {}", id, self.queue.len());
        }
    }

    pub fn leave_queue(&mut self, id: SessionId) {
        self.queue.retain(|&qid| qid != id);
        if let Some(session) = self.sessions.get_mut(&id) {
            if session.state == PlayerState::InQueue {
                session.state = PlayerState::Idle;
            }
        }
        tracing::debug!(
            "Removed {} from queue. Queue size: {}",
            id,
            self.queue.len()
        );
    }

    pub fn try_match(&mut self) -> Option<u64> {
        if self.queue.len() < 2 {
            return None;
        }

        let player1 = self.queue.pop_front()?;
        let player2 = self.queue.pop_front()?;

        let swap = rand::rng().random_bool(0.5);
        let (white_id, black_id) = if swap {
            (player2, player1)
        } else {
            (player1, player2)
        };

        let game_id = self.next_game_id;
        self.next_game_id += 1;

        let game_session = GameSession::new(game_id, white_id, black_id);
        self.games.insert(game_id, game_session);

        let white_name = self
            .sessions
            .get(&white_id)
            .and_then(|s| s.username.clone())
            .unwrap_or_else(|| white_id.to_string());
        let black_name = self
            .sessions
            .get(&black_id)
            .and_then(|s| s.username.clone())
            .unwrap_or_else(|| black_id.to_string());

        if let Some(s) = self.sessions.get_mut(&white_id) {
            s.state = PlayerState::Playing(game_id);
        }
        if let Some(s) = self.sessions.get_mut(&black_id) {
            s.state = PlayerState::Playing(game_id);
        }

        tracing::info!(
            "Game {} started: {} (white) vs {} (black)",
            game_id,
            white_name,
            black_name
        );

        for (id, opponent_name, is_white) in
            [(white_id, black_name, true), (black_id, white_name, false)]
        {
            self.send_event(
                id,
                GameEvent::MatchFound {
                    game_id,
                    opponent_name,
                    is_white,
                },
            );
        }
        Some(game_id)
    }

    pub fn get_match_info(&self, game_id: u64) -> Option<MatchInfo> {
        let game = self.games.get(&game_id)?;

        let white_name = self
            .sessions
            .get(&game.white_player)
            .and_then(|s| s.username.clone())
            .unwrap_or_else(|| game.white_player.to_string());
        let black_name = self
            .sessions
            .get(&game.black_player)
            .and_then(|s| s.username.clone())
            .unwrap_or_else(|| game.black_player.to_string());

        Some(MatchInfo {
            game_id,
            white_id: game.white_player,
            black_id: game.black_player,
            white_name,
            black_name,
        })
    }

    pub fn get_game(&self, game_id: u64) -> Option<&GameSession> {
        self.games.get(&game_id)
    }

    pub fn get_game_mut(&mut self, game_id: u64) -> Option<&mut GameSession> {
        self.games.get_mut(&game_id)
    }

    pub fn get_player_game_id(&self, session_id: SessionId) -> Option<u64> {
        self.sessions.get(&session_id).and_then(|s| {
            if let PlayerState::Playing(gid) = s.state {
                Some(gid)
            } else {
                None
            }
        })
    }

    pub fn get_game_event_tx(
        &self,
        session_id: SessionId,
    ) -> Option<mpsc::UnboundedSender<GameEvent>> {
        self.sessions
            .get(&session_id)
            .and_then(|s| s.game_event_tx.clone())
    }

    fn end_game(&mut self, game_id: u64, reason: &str) -> Option<(SessionId, SessionId)> {
        let info = self.get_match_info(game_id)?;
        let game = self.games.remove(&game_id)?;
        let result = match game.game.result() {
            GameResult::WhiteWins | GameResult::BlackResigned => "1-0",
            GameResult::BlackWins | GameResult::WhiteResigned => "0-1",
            GameResult::Draw => "1/2-1/2",
            GameResult::Ongoing => "*",
        };
        let record = GameRecord {
            id: game_id,
            white_player: info.white_name,
            black_player: info.black_name,
            result: result.to_string(),
            reason: reason.to_string(),
            moves: game.game.san_history().to_vec(),
            final_fen: game.game.fen(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        if let Err(error) = self.history.add_record(record) {
            tracing::error!(
                "Failed to persist game {game_id}; retained in memory for retry: {error}"
            );
        }
        let white_id = game.white_player;
        let black_id = game.black_player;

        if let Some(s) = self.sessions.get_mut(&white_id) {
            s.state = PlayerState::Idle;
        }
        if let Some(s) = self.sessions.get_mut(&black_id) {
            s.state = PlayerState::Idle;
        }

        tracing::info!("Game {} ended", game_id);
        Some((white_id, black_id))
    }

    fn send_event(&self, id: SessionId, event: GameEvent) {
        if let Some(tx) = self.get_game_event_tx(id) {
            let _ = tx.send(event);
        }
    }

    fn publish_state(&self, game_id: u64, finished_reason: Option<String>) {
        if let Some(session) = self.games.get(&game_id) {
            for id in [session.white_player, session.black_player] {
                self.send_event(
                    id,
                    GameEvent::StateUpdated {
                        game_id,
                        game: session.game.clone(),
                        finished_reason: finished_reason.clone(),
                    },
                );
            }
        }
    }

    fn finish_game(&mut self, game_id: u64, reason: &str) {
        self.publish_state(game_id, Some(reason.to_string()));
        self.end_game(game_id, reason);
    }

    pub fn submit_move(&mut self, id: SessionId, game_id: u64, text: &str) -> Result<(), String> {
        let session = self.games.get_mut(&game_id).ok_or("Game is already over")?;
        session.play_move_text(id, text)?;
        if session.game.is_game_over() {
            let reason = if session.game.is_checkmate() {
                "Checkmate"
            } else if session.game.is_stalemate() {
                "Stalemate"
            } else {
                "Insufficient material"
            };
            self.finish_game(game_id, reason);
        } else {
            self.publish_state(game_id, None);
        }
        Ok(())
    }

    pub fn resign_game(&mut self, id: SessionId, game_id: u64) -> Result<(), String> {
        self.games
            .get_mut(&game_id)
            .ok_or("Game is already over")?
            .resign(id)?;
        self.finish_game(game_id, "Resignation");
        Ok(())
    }

    pub fn offer_draw(&mut self, id: SessionId, game_id: u64) -> Result<(), String> {
        let session = self.games.get_mut(&game_id).ok_or("Game is already over")?;
        session.offer_draw(id)?;
        if session.game.is_game_over() {
            self.finish_game(game_id, "Draw by agreement");
        } else if let Some(opponent) = session.get_opponent(id) {
            self.send_event(opponent, GameEvent::DrawOffered { game_id });
        }
        Ok(())
    }

    pub fn history(&self) -> &GameHistory {
        &self.history
    }

    pub fn shutdown(&mut self) -> std::io::Result<()> {
        let ids: Vec<_> = self.games.keys().copied().collect();
        for id in ids {
            self.end_game(id, "Server shutdown");
        }
        self.history.flush()
    }

    pub fn find_by_username(&self, username: &str) -> Option<&PlayerSession> {
        self.sessions
            .values()
            .find(|s| s.username.as_deref() == Some(username))
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{Game, GameResult};

    fn matched() -> (
        SessionManager,
        u64,
        SessionId,
        SessionId,
        mpsc::UnboundedReceiver<GameEvent>,
        mpsc::UnboundedReceiver<GameEvent>,
    ) {
        let mut manager = SessionManager::new();
        let mut receivers = HashMap::new();
        for _ in 0..2 {
            let id = SessionId::new();
            manager.add_session(id, TerminalSize::default());
            let (tx, rx) = mpsc::unbounded_channel();
            manager.register_game_event_channel(id, tx);
            receivers.insert(id, rx);
            manager.join_queue(id);
        }
        let gid = manager.try_match().unwrap();
        let info = manager.get_match_info(gid).unwrap();
        let mut white_rx = receivers.remove(&info.white_id).unwrap();
        let mut black_rx = receivers.remove(&info.black_id).unwrap();
        assert!(matches!(
            white_rx.try_recv().unwrap(),
            GameEvent::MatchFound { is_white: true, .. }
        ));
        assert!(matches!(
            black_rx.try_recv().unwrap(),
            GameEvent::MatchFound {
                is_white: false,
                ..
            }
        ));
        (
            manager,
            gid,
            info.white_id,
            info.black_id,
            white_rx,
            black_rx,
        )
    }

    fn assert_released(manager: &mut SessionManager, gid: u64, white: SessionId, black: SessionId) {
        assert!(manager.get_game(gid).is_none());
        assert_eq!(manager.history().get_records().len(), 1);
        assert_eq!(manager.history().get_records()[0].id, gid);
        for id in [white, black] {
            assert_eq!(manager.get_session(id).unwrap().state, PlayerState::Idle);
            manager.join_queue(id);
        }
        let next = manager.try_match().unwrap();
        assert_ne!(next, gid);
        assert!(manager.get_game(next).is_some());
    }

    #[test]
    fn both_players_receive_identical_final_position_and_can_play_again() {
        for moves in [
            vec!["f3", "e5", "g4", "Qh4#"],
            vec!["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"],
        ] {
            let (mut manager, gid, white, black, mut wrx, mut brx) = matched();
            for (i, mv) in moves.iter().enumerate() {
                manager
                    .submit_move(if i % 2 == 0 { white } else { black }, gid, mv)
                    .unwrap();
                let GameEvent::StateUpdated {
                    game: wg,
                    finished_reason: reason,
                    ..
                } = wrx.try_recv().unwrap()
                else {
                    panic!()
                };
                let GameEvent::StateUpdated { game: bg, .. } = brx.try_recv().unwrap() else {
                    panic!()
                };
                assert_eq!(wg.fen(), bg.fen());
                assert_eq!(wg.san_history(), bg.san_history());
                assert_eq!(reason.is_some(), i == moves.len() - 1);
                if reason.is_some() {
                    assert!(wg.is_checkmate());
                }
            }
            assert!(manager.submit_move(white, gid, "e4").is_err());
            assert_released(&mut manager, gid, white, black);
        }
    }

    #[test]
    fn resign_and_agreed_draw_release_both_players() {
        for draw in [false, true] {
            let (mut manager, gid, white, black, mut wrx, mut brx) = matched();
            if draw {
                manager.offer_draw(white, gid).unwrap();
                assert!(matches!(
                    brx.try_recv().unwrap(),
                    GameEvent::DrawOffered { .. }
                ));
                manager.offer_draw(black, gid).unwrap();
            } else {
                manager.resign_game(white, gid).unwrap();
            }
            for rx in [&mut wrx, &mut brx] {
                let GameEvent::StateUpdated {
                    game,
                    finished_reason,
                    ..
                } = rx.try_recv().unwrap()
                else {
                    panic!()
                };
                assert!(finished_reason.is_some());
                assert_eq!(
                    game.result(),
                    if draw {
                        GameResult::Draw
                    } else {
                        GameResult::WhiteResigned
                    }
                );
                assert!(rx.try_recv().is_err());
            }
            assert_released(&mut manager, gid, white, black);
        }
    }

    #[test]
    fn disconnect_finishes_once_and_releases_opponent() {
        let (mut manager, gid, white, black, _, mut brx) = matched();
        manager.remove_session(white);
        manager.remove_session(white);
        assert!(manager.get_game(gid).is_none());
        assert_eq!(manager.get_session(black).unwrap().state, PlayerState::Idle);
        let GameEvent::StateUpdated {
            game,
            finished_reason,
            ..
        } = brx.try_recv().unwrap()
        else {
            panic!()
        };
        assert_eq!(game.result(), GameResult::WhiteResigned);
        assert_eq!(finished_reason.as_deref(), Some("Opponent disconnected"));
        assert!(brx.try_recv().is_err());
        manager.join_queue(black);
        assert_eq!(manager.queue_size(), 1);
    }

    #[test]
    fn stalemate_and_insufficient_material_finish_normally() {
        for (fen, mv) in [
            ("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", "Qf7"),
            ("7k/8/8/8/8/8/1b6/K7 w - - 0 1", "Kxb2"),
        ] {
            let (mut manager, gid, white, black, mut wrx, _) = matched();
            manager.get_game_mut(gid).unwrap().game = Game::from_fen(fen).unwrap();
            manager.submit_move(white, gid, mv).unwrap();
            let GameEvent::StateUpdated {
                game,
                finished_reason,
                ..
            } = wrx.try_recv().unwrap()
            else {
                panic!()
            };
            assert_eq!(game.result(), GameResult::Draw);
            assert!(finished_reason.is_some());
            assert_released(&mut manager, gid, white, black);
        }
    }

    #[test]
    fn persistence_records_disconnect_and_shutdown_and_resumes_ids() {
        let path =
            std::env::temp_dir().join(format!("chessh-session-history-{}", rand::random::<u64>()));
        let (mut manager, gid, white, black, _, _) = matched();
        manager.history = GameHistory::open(&path).unwrap();
        manager.set_username(white, "Alice".into());
        manager.set_username(black, "Bob".into());
        manager.submit_move(white, gid, "e4").unwrap();
        let fen = manager.get_game(gid).unwrap().game.fen();
        manager.remove_session(white);
        manager.remove_session(white);
        let record = &manager.history().get_records()[0];
        assert_eq!(record.white_player, "Alice");
        assert_eq!(record.black_player, "Bob");
        assert_eq!(record.result, "0-1");
        assert_eq!(record.final_fen, fen);
        assert_eq!(record.moves, ["e4"]);
        manager.add_session(white, TerminalSize::default());
        manager.join_queue(white);
        manager.join_queue(black);
        let next = manager.try_match().unwrap();
        manager.shutdown().unwrap();
        manager.shutdown().unwrap();
        assert_eq!(manager.history().get_records().len(), 2);
        assert!(manager.get_game(next).is_none());
        drop(manager);
        let mut restarted = SessionManager::with_history(GameHistory::open(&path).unwrap());
        let records = restarted.history().get_records();
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].reason, "Server shutdown");
        assert_eq!(records[1].result, "*");
        for id in [white, black] {
            restarted.add_session(id, TerminalSize::default());
            restarted.join_queue(id);
        }
        assert!(restarted.try_match().unwrap() > next);
        drop(restarted);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn queue_rejects_unknown_and_busy_players_and_removes_disconnects() {
        let (mut manager, gid, white, _, _, _) = matched();
        manager.join_queue(white);
        manager.join_queue(SessionId::new());
        manager.leave_queue(white);
        assert_eq!(manager.queue_size(), 0);
        assert_eq!(manager.get_player_game_id(white), Some(gid));
        let id = SessionId::new();
        manager.add_session(id, TerminalSize::default());
        manager.join_queue(id);
        manager.join_queue(id);
        assert_eq!(manager.queue_size(), 1);
        manager.remove_session(id);
        assert_eq!(manager.queue_size(), 0);
    }
}
