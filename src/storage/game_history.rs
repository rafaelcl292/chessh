use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRecord {
    pub id: u64,
    pub white_player: String,
    pub black_player: String,
    pub result: String,
    pub reason: String,
    pub moves: Vec<String>,
    pub final_fen: String,
    pub timestamp: u64,
}

pub struct GameHistory {
    records: Vec<GameRecord>,
    file: Option<File>,
    persisted: usize,
}

impl GameHistory {
    /// In-memory history for embedded callers and tests.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            file: None,
            persisted: 0,
        }
    }

    /// Open an exclusively locked JSON Lines history, recovering an interrupted final append.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;
        file.try_lock().map_err(io::Error::other)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        let mut records = Vec::new();
        let mut offset = 0;
        let mut ids = std::collections::HashSet::new();
        for line in data.split_inclusive(|byte| *byte == b'\n') {
            match serde_json::from_slice::<GameRecord>(line) {
                Ok(record) => {
                    if !ids.insert(record.id) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Duplicate game ID in history",
                        ));
                    }
                    records.push(record);
                    offset += line.len();
                }
                Err(_) if !line.ends_with(b"\n") => {
                    tracing::warn!("Recovering interrupted final history record");
                    file.set_len(offset as u64)?;
                    file.sync_data()?;
                    break;
                }
                Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidData, error)),
            }
        }
        if offset == data.len() && !data.is_empty() && !data.ends_with(b"\n") {
            file.write_all(b"\n")?;
            file.sync_data()?;
        }
        let persisted = records.len();
        Ok(Self {
            records,
            file: Some(file),
            persisted,
        })
    }

    /// Keep a failed append in memory so a later flush can retry it.
    pub fn add_record(&mut self, record: GameRecord) -> io::Result<()> {
        if self.records.iter().any(|existing| existing.id == record.id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Duplicate game ID",
            ));
        }
        self.records.push(record);
        self.flush()
    }

    pub fn flush(&mut self) -> io::Result<()> {
        let Some(file) = &mut self.file else {
            return Ok(());
        };
        while self.persisted < self.records.len() {
            let mut data =
                serde_json::to_vec(&self.records[self.persisted]).map_err(io::Error::other)?;
            data.push(b'\n');
            let start = file.metadata()?.len();
            if let Err(error) = file.write_all(&data).and_then(|_| file.sync_data()) {
                // Prevent a partial line from corrupting the next retry.
                file.set_len(start)?;
                return Err(error);
            }
            self.persisted += 1;
        }
        Ok(())
    }

    pub fn next_id(&self) -> u64 {
        self.records
            .iter()
            .map(|r| r.id)
            .max()
            .map_or(1, |id| id.saturating_add(1))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: u64) -> GameRecord {
        GameRecord {
            id,
            white_player: "white".into(),
            black_player: "black".into(),
            result: "0-1".into(),
            reason: "Checkmate".into(),
            moves: vec!["f3".into(), "e5".into(), "g4".into(), "Qh4#".into()],
            final_fen: "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3".into(),
            timestamp: 1,
        }
    }

    #[test]
    fn history_survives_restart_and_recovers_partial_tail() {
        let path = std::env::temp_dir().join(format!("chessh-history-{}", rand::random::<u64>()));
        {
            let mut history = GameHistory::open(&path).unwrap();
            history.add_record(record(1)).unwrap();
            assert!(
                GameHistory::open(&path).is_err(),
                "only one writer may own a history"
            );
        }
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"id\":2")
            .unwrap();
        {
            let mut history = GameHistory::open(&path).unwrap();
            assert_eq!(history.get_records(), &[record(1)]);
            assert_eq!(history.next_id(), 2);
            history.add_record(record(2)).unwrap();
            assert!(history.add_record(record(2)).is_err());
        }
        let history = GameHistory::open(&path).unwrap();
        assert_eq!(history.get_records(), &[record(1), record(2)]);
        assert_eq!(history.get_player_games("white").len(), 2);
        assert_eq!(history.next_id(), 3);
        drop(history);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn games_may_finish_out_of_order_without_reusing_ids() {
        let path = std::env::temp_dir().join(format!("chessh-history-{}", rand::random::<u64>()));
        {
            let mut history = GameHistory::open(&path).unwrap();
            history.add_record(record(2)).unwrap();
            history.add_record(record(1)).unwrap();
        }
        let history = GameHistory::open(&path).unwrap();
        assert_eq!(history.next_id(), 3);
        assert_eq!(history.get_records(), &[record(2), record(1)]);
        drop(history);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_writes_remain_pending_and_can_be_retried() {
        let path = std::env::temp_dir().join(format!("chessh-history-{}", rand::random::<u64>()));
        let mut history = GameHistory::open(&path).unwrap();
        history.file = Some(File::open(&path).unwrap()); // Simulate an unwritable destination.
        assert!(history.add_record(record(1)).is_err());
        assert_eq!(history.get_records(), &[record(1)]);
        assert_eq!(history.persisted, 0);
        history.file = Some(OpenOptions::new().append(true).open(&path).unwrap());
        history.flush().unwrap();
        history.flush().unwrap();
        drop(history);
        assert_eq!(
            GameHistory::open(&path).unwrap().get_records(),
            &[record(1)]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn complete_corrupt_lines_are_reported_without_modifying_history() {
        let path = std::env::temp_dir().join(format!("chessh-history-{}", rand::random::<u64>()));
        std::fs::write(&path, "bad history\n").unwrap();
        assert!(GameHistory::open(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "bad history\n");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn valid_final_record_without_newline_is_preserved() {
        let path = std::env::temp_dir().join(format!("chessh-history-{}", rand::random::<u64>()));
        std::fs::write(&path, serde_json::to_vec(&record(1)).unwrap()).unwrap();
        {
            let mut history = GameHistory::open(&path).unwrap();
            history.add_record(record(2)).unwrap();
        }
        assert_eq!(GameHistory::open(&path).unwrap().get_records().len(), 2);
        std::fs::remove_file(path).unwrap();
    }
}
