use shakmaty::Color;

use crate::chess::{Game, GameResult};
use crate::ssh::session::SessionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawOfferState {
    None,
    OfferedBy(SessionId),
}

pub struct GameSession {
    pub id: u64,
    pub white_player: SessionId,
    pub black_player: SessionId,
    pub game: Game,
    pub draw_offer: DrawOfferState,
}

impl GameSession {
    pub fn new(id: u64, white_player: SessionId, black_player: SessionId) -> Self {
        Self {
            id,
            white_player,
            black_player,
            game: Game::new(),
            draw_offer: DrawOfferState::None,
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

    pub fn is_players_turn(&self, session_id: SessionId) -> bool {
        let current_turn = self.game.turn();
        match current_turn {
            Color::White => session_id == self.white_player,
            Color::Black => session_id == self.black_player,
        }
    }

    pub fn get_player_color(&self, session_id: SessionId) -> Option<Color> {
        if session_id == self.white_player {
            Some(Color::White)
        } else if session_id == self.black_player {
            Some(Color::Black)
        } else {
            None
        }
    }

    pub fn play_uci(&mut self, session_id: SessionId, uci: &str) -> Result<String, String> {
        if !self.is_players_turn(session_id) {
            return Err("Not your turn".to_string());
        }

        self.game.play_uci(uci)?;
        self.draw_offer = DrawOfferState::None;
        Ok(uci.to_string())
    }

    pub fn play_move_text(&mut self, session_id: SessionId, text: &str) -> Result<String, String> {
        if !self.is_players_turn(session_id) {
            return Err("Not your turn".to_string());
        }

        let normalized = Self::normalize_san(text);
        if self.game.play_san(&normalized).is_ok() {
            self.draw_offer = DrawOfferState::None;
            return Ok(self
                .game
                .move_history()
                .last()
                .expect("successful move")
                .to_uci(shakmaty::CastlingMode::Standard)
                .to_string());
        }

        let uci = text.to_lowercase();
        if self.game.play_uci(&uci).is_ok() {
            self.draw_offer = DrawOfferState::None;
            return Ok(uci);
        }

        Err(format!("Invalid move: {}", text))
    }

    fn normalize_san(input: &str) -> String {
        let input = input.trim();
        if input.is_empty() {
            return input.to_string();
        }

        let first_char = input.chars().next().unwrap();
        if "nbrqkNBRQK".contains(first_char) {
            let mut result = first_char.to_uppercase().to_string();
            result.push_str(&input[1..]);
            result
        } else if input == "o-o" || input == "0-0" {
            "O-O".to_string()
        } else if input == "o-o-o" || input == "0-0-0" {
            "O-O-O".to_string()
        } else {
            input.to_string()
        }
    }

    pub fn offer_draw(&mut self, session_id: SessionId) -> Result<(), String> {
        if !self.is_player(session_id) {
            return Err("Not a player".to_string());
        }

        match self.draw_offer {
            DrawOfferState::None => {
                self.draw_offer = DrawOfferState::OfferedBy(session_id);
                Ok(())
            }
            DrawOfferState::OfferedBy(offerer) if offerer != session_id => {
                self.game.set_result(GameResult::Draw);
                self.draw_offer = DrawOfferState::None;
                Ok(())
            }
            _ => Err("You already offered a draw".to_string()),
        }
    }

    pub fn decline_draw(&mut self, session_id: SessionId) -> Result<(), String> {
        match self.draw_offer {
            DrawOfferState::OfferedBy(offerer) if offerer != session_id => {
                self.draw_offer = DrawOfferState::None;
                Ok(())
            }
            _ => Err("No draw offer to decline".to_string()),
        }
    }

    pub fn resign(&mut self, session_id: SessionId) -> Result<(), String> {
        if let Some(color) = self.get_player_color(session_id) {
            self.game.resign(color);
            Ok(())
        } else {
            Err("Not a player".to_string())
        }
    }

    pub fn has_pending_draw_offer_for(&self, session_id: SessionId) -> bool {
        match self.draw_offer {
            DrawOfferState::OfferedBy(offerer) => offerer != session_id,
            DrawOfferState::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn san_promotions_replay_identically_including_underpromotions() {
        for (san, uci) in [
            ("a8=Q+", "a7a8q"),
            ("a8=R+", "a7a8r"),
            ("a8=B", "a7a8b"),
            ("a8=N", "a7a8n"),
        ] {
            let white = SessionId::new();
            let mut session = GameSession::new(1, white, SessionId::new());
            session.game = Game::from_fen("7k/P7/8/8/8/8/8/7K w - - 0 1").unwrap();
            let mut replica = Game::from_fen(&session.game.fen()).unwrap();
            let played = session.play_move_text(white, san).unwrap();
            assert_eq!(played, uci);
            replica.play_uci(&played).unwrap();
            assert_eq!(replica.fen(), session.game.fen());
        }
    }

    #[test]
    fn castling_replays_using_standard_uci() {
        for (san, uci) in [("O-O", "e1g1"), ("O-O-O", "e1c1")] {
            let white = SessionId::new();
            let mut session = GameSession::new(1, white, SessionId::new());
            session.game = Game::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
            let mut replica = Game::from_fen(&session.game.fen()).unwrap();
            let played = session.play_move_text(white, san).unwrap();
            assert_eq!(played, uci);
            replica.play_uci(&played).unwrap();
            assert_eq!(replica.fen(), session.game.fen());
        }
    }
}
