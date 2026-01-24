use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecord {
    pub id: u64,
    pub white_player: String,
    pub black_player: String,
    pub result: String,
    pub moves: Vec<String>,
    pub timestamp: u64,
}

pub struct GameHistory {
    records: Vec<GameRecord>,
}

impl GameHistory {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn add_record(&mut self, record: GameRecord) {
        self.records.push(record);
    }

    pub fn get_records(&self) -> &[GameRecord] {
        &self.records
    }

    pub fn get_player_games(&self, player: &str) -> Vec<&GameRecord> {
        self.records
            .iter()
            .filter(|r| r.white_player == player || r.black_player == player)
            .collect()
    }
}

impl Default for GameHistory {
    fn default() -> Self {
        Self::new()
    }
}
