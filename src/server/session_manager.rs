use std::collections::HashMap;

use crate::ssh::session::{SessionId, TerminalSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,
    InQueue,
    Playing,
    Spectating,
}

pub struct PlayerSession {
    pub id: SessionId,
    pub username: Option<String>,
    pub state: PlayerState,
    pub terminal_size: TerminalSize,
}

pub struct SessionManager {
    sessions: HashMap<SessionId, PlayerSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn add_session(&mut self, id: SessionId, terminal_size: TerminalSize) {
        let session = PlayerSession {
            id,
            username: None,
            state: PlayerState::Idle,
            terminal_size,
        };
        self.sessions.insert(id, session);
        tracing::info!("Session {} connected. Total: {}", id, self.sessions.len());
    }

    pub fn remove_session(&mut self, id: SessionId) {
        self.sessions.remove(&id);
        tracing::info!("Session {} disconnected. Total: {}", id, self.sessions.len());
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

    pub fn online_count(&self) -> usize {
        self.sessions.len()
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
