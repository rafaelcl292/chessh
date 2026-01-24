use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct LobbyView {
    online_count: usize,
    queue_size: usize,
    username: Option<String>,
}

impl LobbyView {
    pub fn new() -> Self {
        Self {
            online_count: 0,
            queue_size: 0,
            username: None,
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
}

impl Default for LobbyView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for LobbyView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = "♔ ChesSSH ♚";

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                title,
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(format!("Players online: {}", self.online_count)),
            Line::from(format!("In queue: {}", self.queue_size)),
            Line::from(""),
            Line::from("Commands:"),
            Line::from("  /play   - Join matchmaking queue"),
            Line::from("  /quit   - Disconnect"),
            Line::from("  /help   - Show all commands"),
            Line::from(""),
        ];

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Lobby"))
            .alignment(Alignment::Center);

        paragraph.render(area, buf);
    }
}
