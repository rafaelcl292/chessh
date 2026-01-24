use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Widget};
use shakmaty::{Chess, Move};

use super::board::BoardWidget;

pub struct GameView<'a> {
    position: &'a Chess,
    move_history: &'a [Move],
    white_name: String,
    black_name: String,
    status_message: Option<String>,
    selected_square: Option<shakmaty::Square>,
    last_move: Option<(shakmaty::Square, shakmaty::Square)>,
    is_black_perspective: bool,
}

impl<'a> GameView<'a> {
    pub fn new(position: &'a Chess, move_history: &'a [Move]) -> Self {
        Self {
            position,
            move_history,
            white_name: "White".to_string(),
            black_name: "Black".to_string(),
            status_message: None,
            selected_square: None,
            last_move: None,
            is_black_perspective: false,
        }
    }

    pub fn players(mut self, white: String, black: String) -> Self {
        self.white_name = white;
        self.black_name = black;
        self
    }

    pub fn status(mut self, message: String) -> Self {
        self.status_message = Some(message);
        self
    }

    pub fn selected(mut self, square: Option<shakmaty::Square>) -> Self {
        self.selected_square = square;
        self
    }

    pub fn last_move(mut self, from: shakmaty::Square, to: shakmaty::Square) -> Self {
        self.last_move = Some((from, to));
        self
    }

    pub fn perspective(mut self, is_black: bool) -> Self {
        self.is_black_perspective = is_black;
        self
    }
}

impl Widget for GameView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(30)])
            .split(area);

        let board_area = chunks[0];
        let side_panel = chunks[1];

        let mut board = BoardWidget::new(self.position)
            .selected(self.selected_square)
            .flipped(self.is_black_perspective);

        if let Some((from, to)) = self.last_move {
            board = board.last_move(from, to);
        }

        board.render(board_area, buf);

        let side_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(side_panel);

        let top_player = if self.is_black_perspective {
            &self.white_name
        } else {
            &self.black_name
        };
        let bottom_player = if self.is_black_perspective {
            &self.black_name
        } else {
            &self.white_name
        };

        let top_player_widget = Paragraph::new(top_player.as_str())
            .block(Block::default().borders(Borders::ALL));
        top_player_widget.render(side_chunks[0], buf);

        let moves: Vec<ListItem> = self
            .move_history
            .chunks(2)
            .enumerate()
            .map(|(i, pair)| {
                let move_num = i + 1;
                let white_move = pair.first().map(|m| m.to_string()).unwrap_or_default();
                let black_move = pair.get(1).map(|m| m.to_string()).unwrap_or_default();
                ListItem::new(format!("{}. {} {}", move_num, white_move, black_move))
            })
            .collect();

        let moves_list = List::new(moves)
            .block(Block::default().borders(Borders::ALL).title("Moves"));
        moves_list.render(side_chunks[1], buf);

        let bottom_player_widget = Paragraph::new(bottom_player.as_str())
            .block(Block::default().borders(Borders::ALL));
        bottom_player_widget.render(side_chunks[2], buf);

        if let Some(status) = self.status_message {
            let status_widget = Paragraph::new(status)
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            status_widget.render(side_chunks[3], buf);
        }
    }
}
