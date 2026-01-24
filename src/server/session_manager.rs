use std::collections::{HashMap, VecDeque};

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
    queue: VecDeque<SessionId>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            queue: VecDeque::new(),
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
        self.queue.retain(|&qid| qid != id);
        self.sessions.remove(&id);
        tracing::info!(
            "Session {} disconnected. Total: {}",
            id,
            self.sessions.len()
        );
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

    pub fn try_match(&mut self) -> Option<(SessionId, SessionId)> {
        if self.queue.len() >= 2 {
            let player1 = self.queue.pop_front()?;
            let player2 = self.queue.pop_front()?;
            if let Some(s) = self.sessions.get_mut(&player1) {
                s.state = PlayerState::Playing;
            }
            if let Some(s) = self.sessions.get_mut(&player2) {
                s.state = PlayerState::Playing;
            }
            tracing::info!("Matched {} vs {}", player1, player2);
            Some((player1, player2))
        } else {
            None
        }
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
