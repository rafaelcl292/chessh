use std::collections::{HashMap, VecDeque};

use rand::Rng;
use tokio::sync::mpsc;

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
    pub game_event_tx: Option<mpsc::Sender<GameEvent>>,
}

pub struct SessionManager {
    sessions: HashMap<SessionId, PlayerSession>,
    queue: VecDeque<SessionId>,
    games: HashMap<u64, GameSession>,
    next_game_id: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            queue: VecDeque::new(),
            games: HashMap::new(),
            next_game_id: 1,
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

    pub fn register_game_event_channel(&mut self, id: SessionId, tx: mpsc::Sender<GameEvent>) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.game_event_tx = Some(tx);
        }
    }

    pub fn remove_session(&mut self, id: SessionId) -> Option<u64> {
        self.queue.retain(|&qid| qid != id);

        let game_id = if let Some(session) = self.sessions.get(&id) {
            if let PlayerState::Playing(gid) = session.state {
                Some(gid)
            } else {
                None
            }
        } else {
            None
        };

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
        if !self.queue.contains(&id) {
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
            session.state = PlayerState::Idle;
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

    pub fn get_game_event_tx(&self, session_id: SessionId) -> Option<mpsc::Sender<GameEvent>> {
        self.sessions
            .get(&session_id)
            .and_then(|s| s.game_event_tx.clone())
    }

    pub fn end_game(&mut self, game_id: u64) -> Option<(SessionId, SessionId)> {
        let game = self.games.remove(&game_id)?;
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
