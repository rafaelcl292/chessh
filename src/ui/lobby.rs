use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct LobbyView {
    online_count: usize,
    queue_size: usize,
    username: Option<String>,
    searching: bool,
}

impl LobbyView {
    pub fn new() -> Self {
        Self {
            online_count: 0,
            queue_size: 0,
            username: None,
            searching: false,
        }
    }

    pub fn online_count(mut self, count: usize) -> Self {
        self.online_count = count;
        self
    }

    pub fn queue_size(mut self, size: usize) -> Self {
        self.queue_size = size;
        self
    }

    pub fn username(mut self, name: Option<String>) -> Self {
        self.username = name;
        self
    }

    pub fn searching(mut self, searching: bool) -> Self {
        self.searching = searching;
        self
    }
}

impl Default for LobbyView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for LobbyView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = "♔ ChesSSH ♚";

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(title, Style::default().fg(Color::Yellow))),
            Line::from(""),
            Line::from(format!("Players online: {}", self.online_count)),
            Line::from(format!("In queue: {}", self.queue_size)),
            Line::from(""),
        ];

        if self.searching {
            lines.push(Line::from(Span::styled(
                "Searching for opponent... (ESC to cancel)",
                Style::default().fg(Color::Green),
            )));
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from("Commands:"));
            lines.push(Line::from("  /play  - Join matchmaking queue"));
            lines.push(Line::from("  /solo  - Start a solo game"));
            lines.push(Line::from("  /quit  - Disconnect"));
            lines.push(Line::from(""));
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Lobby"))
            .alignment(Alignment::Center);

        paragraph.render(area, buf);
    }
}
