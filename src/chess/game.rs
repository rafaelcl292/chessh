use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::{Chess, Color, KnownOutcome, Move, Outcome, Position, Square};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Ongoing,
    WhiteWins,
    BlackWins,
    Draw,
    WhiteResigned,
    BlackResigned,
}

pub struct Game {
    position: Chess,
    move_history: Vec<Move>,
    san_history: Vec<String>,
    result: GameResult,
}

impl Game {
    pub fn new() -> Self {
        Self {
            position: Chess::default(),
            move_history: Vec::new(),
            san_history: Vec::new(),
            result: GameResult::Ongoing,
        }
    }

    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let fen: Fen = fen.parse().map_err(|e| format!("Invalid FEN: {}", e))?;
        let position: Chess = fen
            .into_position(shakmaty::CastlingMode::Standard)
            .map_err(|e| format!("Invalid position: {}", e))?;
        Ok(Self {
            position,
            move_history: Vec::new(),
            san_history: Vec::new(),
            result: GameResult::Ongoing,
        })
    }

    pub fn play_san(&mut self, san_str: &str) -> Result<(), String> {
        if self.result != GameResult::Ongoing {
            return Err("Game is already over".to_string());
        }

        let san: San = san_str.parse().map_err(|e| format!("Invalid SAN: {}", e))?;

        let mv = san
            .to_move(&self.position)
            .map_err(|e| format!("Illegal move: {}", e))?;

        let san_notation = San::from_move(&self.position, mv.clone()).to_string();

        self.position = self
            .position
            .clone()
            .play(mv.clone())
            .map_err(|e| format!("{}", e))?;
        self.move_history.push(mv);
        self.san_history.push(san_notation);

        self.update_result();
        Ok(())
    }

    pub fn play_uci(&mut self, uci_str: &str) -> Result<(), String> {
        if self.result != GameResult::Ongoing {
            return Err("Game is already over".to_string());
        }

        let uci_str = uci_str.trim();
        if uci_str.len() < 4 {
            return Err("Invalid UCI notation".to_string());
        }

        let from = uci_str[0..2]
            .parse::<Square>()
            .map_err(|_| "Invalid from square")?;
        let to = uci_str[2..4]
            .parse::<Square>()
            .map_err(|_| "Invalid to square")?;

        let promotion = if uci_str.len() > 4 {
            match uci_str.chars().nth(4) {
                Some('q') => Some(shakmaty::Role::Queen),
                Some('r') => Some(shakmaty::Role::Rook),
                Some('b') => Some(shakmaty::Role::Bishop),
                Some('n') => Some(shakmaty::Role::Knight),
                _ => None,
            }
        } else {
            None
        };

        let legal_moves = self.legal_moves();
        let mv = legal_moves
            .into_iter()
            .find(|m| m.from() == Some(from) && m.to() == to && m.promotion() == promotion)
            .ok_or_else(|| "Illegal move".to_string())?;

        let san_notation = San::from_move(&self.position, mv.clone()).to_string();

        self.position = self
            .position
            .clone()
            .play(mv.clone())
            .map_err(|e| format!("{}", e))?;
        self.move_history.push(mv);
        self.san_history.push(san_notation);

        self.update_result();
        Ok(())
    }

    pub fn play_move(&mut self, mv: Move) -> Result<(), String> {
        if self.result != GameResult::Ongoing {
            return Err("Game is already over".to_string());
        }

        if !self.is_legal(&mv) {
            return Err("Illegal move".to_string());
        }

        let san_notation = San::from_move(&self.position, mv.clone()).to_string();

        self.position = self
            .position
            .clone()
            .play(mv.clone())
            .map_err(|e| format!("{}", e))?;
        self.move_history.push(mv);
        self.san_history.push(san_notation);

        self.update_result();
        Ok(())
    }

    fn update_result(&mut self) {
        match self.position.outcome() {
            Outcome::Known(known) => match known {
                KnownOutcome::Decisive { winner } => {
                    self.result = match winner {
                        Color::White => GameResult::WhiteWins,
                        Color::Black => GameResult::BlackWins,
                    };
                }
                KnownOutcome::Draw => {
                    self.result = GameResult::Draw;
                }
            },
            Outcome::Unknown => {}
        }
    }

    pub fn resign(&mut self, color: Color) {
        self.result = match color {
            Color::White => GameResult::WhiteResigned,
            Color::Black => GameResult::BlackResigned,
        };
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        self.position.legal_moves().into_iter().collect()
    }

    pub fn is_legal(&self, mv: &Move) -> bool {
        self.position.is_legal(mv.clone())
    }

    pub fn is_check(&self) -> bool {
        self.position.is_check()
    }

    pub fn is_checkmate(&self) -> bool {
        self.position.is_checkmate()
    }

    pub fn is_stalemate(&self) -> bool {
        self.position.is_stalemate()
    }

    pub fn is_game_over(&self) -> bool {
        self.result != GameResult::Ongoing || self.position.is_game_over()
    }

    pub fn outcome(&self) -> Outcome {
        self.position.outcome()
    }

    pub fn turn(&self) -> Color {
        self.position.turn()
    }

    pub fn fen(&self) -> String {
        shakmaty::fen::Fen::from_position(&self.position, shakmaty::EnPassantMode::Legal)
            .to_string()
    }

    pub fn result(&self) -> GameResult {
        self.result
    }

    pub fn move_history(&self) -> &[Move] {
        &self.move_history
    }

    pub fn san_history(&self) -> &[String] {
        &self.san_history
    }

    pub fn position(&self) -> &Chess {
        &self.position
    }

    pub fn move_count(&self) -> usize {
        self.move_history.len()
    }

    pub fn format_move_san(&self, mv: &Move) -> String {
        San::from_move(&self.position, mv.clone()).to_string()
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
