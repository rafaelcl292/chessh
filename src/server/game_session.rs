use crate::chess::Game;
use crate::ssh::session::SessionId;

pub struct GameSession {
    pub id: u64,
    pub white_player: SessionId,
    pub black_player: SessionId,
    pub game: Game,
}

impl GameSession {
    pub fn new(id: u64, white_player: SessionId, black_player: SessionId) -> Self {
        Self {
            id,
            white_player,
            black_player,
            game: Game::new(),
        }
    }

    pub fn is_player(&self, session_id: SessionId) -> bool {
        self.white_player == session_id || self.black_player == session_id
    }

    pub fn get_opponent(&self, session_id: SessionId) -> Option<SessionId> {
        if session_id == self.white_player {
            Some(self.black_player)
        } else if session_id == self.black_player {
            Some(self.white_player)
        } else {
            None
        }
    }
}
