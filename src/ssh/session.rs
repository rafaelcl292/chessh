use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

use crate::chess::Game;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub enum GameEvent {
    MatchFound {
        game_id: u64,
        opponent_name: String,
        is_white: bool,
    },
    StateUpdated {
        game_id: u64,
        game: Game,
        finished_reason: Option<String>,
    },
    DrawOffered {
        game_id: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u64);

impl SessionId {
    pub fn new() -> Self {
        Self(SESSION_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

pub struct TerminalSize {
    pub width: u32,
    pub height: u32,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
        }
    }
}

pub enum SessionEvent {
    Data(Vec<u8>),
    Resize(TerminalSize),
    Disconnect,
}

pub struct Session {
    pub id: SessionId,
    pub username: Option<String>,
    pub terminal_size: TerminalSize,
    pub event_tx: mpsc::Sender<SessionEvent>,
    pub event_rx: mpsc::Receiver<SessionEvent>,
}

impl Session {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(256);
        Self {
            id: SessionId::new(),
            username: None,
            terminal_size: TerminalSize::default(),
            event_tx,
            event_rx,
        }
    }

    pub fn set_terminal_size(&mut self, width: u32, height: u32) {
        self.terminal_size = TerminalSize { width, height };
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
