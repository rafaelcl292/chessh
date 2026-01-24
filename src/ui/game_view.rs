use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Widget};
use shakmaty::{Chess, Position, Role};

use super::board::BoardWidget;

pub struct GameView<'a> {
    position: &'a Chess,
    san_history: &'a [String],
    white_name: String,
    black_name: String,
    status_message: Option<String>,
    selected_square: Option<shakmaty::Square>,
    last_move: Option<(shakmaty::Square, shakmaty::Square)>,
    is_black_perspective: bool,
    highlight_origins: Vec<shakmaty::Square>,
    highlight_destinations: Vec<shakmaty::Square>,
}

fn captured_pieces(position: &Chess, color: shakmaty::Color) -> Vec<Role> {
    let starting_counts = [
        (Role::Pawn, 8),
        (Role::Knight, 2),
        (Role::Bishop, 2),
        (Role::Rook, 2),
        (Role::Queen, 1),
    ];

    let mut captured = Vec::new();
    let board = position.board();

    for (role, start_count) in starting_counts {
        let current = board.by_piece(shakmaty::Piece { color, role }).count();
        for _ in 0..(start_count - current) {
            captured.push(role);
        }
    }

    captured
}

fn role_to_symbol(role: Role, color: shakmaty::Color) -> char {
    match (color, role) {
        (shakmaty::Color::White, Role::King) => '♔',
        (shakmaty::Color::White, Role::Queen) => '♕',
        (shakmaty::Color::White, Role::Rook) => '♖',
        (shakmaty::Color::White, Role::Bishop) => '♗',
        (shakmaty::Color::White, Role::Knight) => '♘',
        (shakmaty::Color::White, Role::Pawn) => '♙',
        (shakmaty::Color::Black, Role::King) => '♚',
        (shakmaty::Color::Black, Role::Queen) => '♛',
        (shakmaty::Color::Black, Role::Rook) => '♜',
        (shakmaty::Color::Black, Role::Bishop) => '♝',
        (shakmaty::Color::Black, Role::Knight) => '♞',
        (shakmaty::Color::Black, Role::Pawn) => '♟',
    }
}

impl<'a> GameView<'a> {
    pub fn new(position: &'a Chess, san_history: &'a [String]) -> Self {
        Self {
            position,
            san_history,
            white_name: "White".to_string(),
            black_name: "Black".to_string(),
            status_message: None,
            selected_square: None,
            last_move: None,
            is_black_perspective: false,
            highlight_origins: Vec::new(),
            highlight_destinations: Vec::new(),
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

    pub fn highlights(
        mut self,
        origins: Vec<shakmaty::Square>,
        destinations: Vec<shakmaty::Square>,
    ) -> Self {
        self.highlight_origins = origins;
        self.highlight_destinations = destinations;
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
            .flipped(self.is_black_perspective)
            .highlights(self.highlight_origins, self.highlight_destinations);

        if let Some((from, to)) = self.last_move {
            board = board.last_move(from, to);
        }

        board.render(board_area, buf);

        let side_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(8),
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(side_panel);

        let (top_color, bottom_color) = if self.is_black_perspective {
            (shakmaty::Color::White, shakmaty::Color::Black)
        } else {
            (shakmaty::Color::Black, shakmaty::Color::White)
        };

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

        let is_top_turn = self.position.turn() == top_color;
        let is_bottom_turn = self.position.turn() == bottom_color;

        let top_style = if is_top_turn {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let top_player_widget = Paragraph::new(top_player.as_str())
            .style(top_style)
            .block(Block::default().borders(Borders::ALL));
        top_player_widget.render(side_chunks[0], buf);

        let top_captured = captured_pieces(self.position, top_color);
        let top_captured_str: String = top_captured
            .iter()
            .map(|&r| role_to_symbol(r, top_color))
            .collect();
        let top_captured_widget =
            Paragraph::new(top_captured_str).style(Style::default().fg(Color::Gray));
        top_captured_widget.render(side_chunks[1], buf);

        let moves: Vec<ListItem> = self
            .san_history
            .chunks(2)
            .enumerate()
            .map(|(i, pair)| {
                let move_num = i + 1;
                let white_move = pair.first().map(|s| s.as_str()).unwrap_or("");
                let black_move = pair.get(1).map(|s| s.as_str()).unwrap_or("");
                ListItem::new(format!("{}. {} {}", move_num, white_move, black_move))
            })
            .collect();

        let moves_list =
            List::new(moves).block(Block::default().borders(Borders::ALL).title("Moves"));
        moves_list.render(side_chunks[2], buf);

        let bottom_captured = captured_pieces(self.position, bottom_color);
        let bottom_captured_str: String = bottom_captured
            .iter()
            .map(|&r| role_to_symbol(r, bottom_color))
            .collect();
        let bottom_captured_widget =
            Paragraph::new(bottom_captured_str).style(Style::default().fg(Color::Gray));
        bottom_captured_widget.render(side_chunks[3], buf);

        let bottom_style = if is_bottom_turn {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let bottom_player_widget = Paragraph::new(bottom_player.as_str())
            .style(bottom_style)
            .block(Block::default().borders(Borders::ALL));
        bottom_player_widget.render(side_chunks[4], buf);

        if let Some(status) = self.status_message {
            let status_style = if status.contains("Check") {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if status.contains("Checkmate") || status.contains("wins") {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let status_widget = Paragraph::new(status)
                .style(status_style)
                .block(Block::default().borders(Borders::ALL).title("Status"));
            status_widget.render(side_chunks[5], buf);
        }
    }
}
