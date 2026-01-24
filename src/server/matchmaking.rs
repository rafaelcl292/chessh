use std::collections::VecDeque;

use crate::ssh::session::SessionId;

pub struct Matchmaking {
    queue: VecDeque<SessionId>,
}

impl Matchmaking {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn add_to_queue(&mut self, session_id: SessionId) {
        if !self.queue.contains(&session_id) {
            self.queue.push_back(session_id);
            tracing::debug!("Added {} to queue. Queue size: {}", session_id, self.queue.len());
        }
    }

    pub fn remove_from_queue(&mut self, session_id: SessionId) {
        self.queue.retain(|&id| id != session_id);
    }

    pub fn try_match(&mut self) -> Option<(SessionId, SessionId)> {
        if self.queue.len() >= 2 {
            let player1 = self.queue.pop_front()?;
            let player2 = self.queue.pop_front()?;
            tracing::info!("Matched {} vs {}", player1, player2);
            Some((player1, player2))
        } else {
            None
        }
    }

    pub fn queue_position(&self, session_id: SessionId) -> Option<usize> {
        self.queue.iter().position(|&id| id == session_id)
    }

    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }
}

impl Default for Matchmaking {
    fn default() -> Self {
        Self::new()
    }
}
