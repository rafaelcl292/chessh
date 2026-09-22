use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph, Widget};
use shakmaty::{Chess, Position, Role};

use super::board::BoardWidget;
use super::controls::{self, panel, GameControl, ACCENT, BG, MUTED, TEXT};

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
    input_buffer: String,
    is_my_turn: bool,
    is_multiplayer: bool,
    computer: bool,
    finished: Option<String>,
}

fn captured_pieces(position: &Chess, color: shakmaty::Color) -> Vec<Role> {
    let starting_counts = [
        (Role::Pawn, 8usize),
        (Role::Knight, 2),
        (Role::Bishop, 2),
        (Role::Rook, 2),
        (Role::Queen, 1),
    ];

    let mut captured = Vec::new();
    let board = position.board();

    for (role, start_count) in starting_counts {
        let current = board.by_piece(shakmaty::Piece { color, role }).count();
        for _ in 0..start_count.saturating_sub(current) {
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
            input_buffer: String::new(),
            is_my_turn: true,
            is_multiplayer: false,
            computer: false,
            finished: None,
        }
    }

    pub fn finished(mut self, result: String) -> Self {
        self.finished = Some(result);
        self
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

    pub fn computer(mut self, value: bool) -> Self {
        self.computer = value;
        self
    }

    pub fn input(mut self, buffer: String, is_my_turn: bool, is_multiplayer: bool) -> Self {
        self.input_buffer = buffer;
        self.is_my_turn = is_my_turn;
        self.is_multiplayer = is_multiplayer;
        self
    }

    pub fn board_area(area: Rect) -> Rect {
        let area = Self::content_area(area);
        let (help, side, _, _) = Self::compute_layout_widths(area, 18, 26, 34);
        let mut constraints = Vec::new();
        if help {
            constraints.push(Constraint::Length(18));
        }
        constraints.push(Constraint::Min(34));
        if side {
            constraints.push(Constraint::Length(26));
        }
        Layout::horizontal(constraints).split(area)[usize::from(help)]
    }

    fn content_area(area: Rect) -> Rect {
        if area.width < 78 {
            Rect::new(area.x, area.y, area.width, area.height.saturating_sub(3))
        } else {
            area
        }
    }

    pub fn get_input_position(&self, area: Rect) -> (u16, u16) {
        let area = Self::content_area(area);
        let help_width = 18u16;
        let side_width = 26u16;
        let min_board_width = 34u16;

        let (has_help, has_side, _, _) =
            Self::compute_layout_widths(area, help_width, side_width, min_board_width);

        let mut constraints = Vec::new();
        if has_help {
            constraints.push(Constraint::Length(help_width));
        }
        constraints.push(Constraint::Min(min_board_width));
        if has_side {
            constraints.push(Constraint::Length(side_width));
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        if !has_side {
            return (area.x + 1, area.y + area.height.saturating_sub(1));
        }

        let side_panel = if has_help { chunks[2] } else { chunks[1] };

        let side_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(5),
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(side_panel);

        let input_area = side_chunks[6];
        let prefix_len = 3u16;
        let cursor_x = input_area.x + prefix_len + self.input_buffer.len() as u16;
        let cursor_y = input_area.y + 1;

        (cursor_x.min(input_area.right().saturating_sub(2)), cursor_y)
    }

    fn compute_layout_widths(
        area: Rect,
        help_width: u16,
        side_width: u16,
        min_board_width: u16,
    ) -> (bool, bool, u16, u16) {
        let total = area.width;
        let full_width = help_width + min_board_width + side_width;
        let no_help_width = min_board_width + side_width;

        if total >= full_width {
            (true, true, help_width, side_width)
        } else if total >= no_help_width {
            (false, true, 0, side_width)
        } else {
            (false, false, 0, 0)
        }
    }
}

impl Widget for GameView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let help_width = 18u16;
        let side_width = 26u16;
        let min_board_width = 34u16;

        let (has_help, has_side, _, _) =
            Self::compute_layout_widths(area, help_width, side_width, min_board_width);

        let full_area = area;
        Block::default()
            .style(Style::default().bg(BG).fg(TEXT))
            .render(area, buf);
        let area = Self::content_area(area);

        let mut constraints = Vec::new();
        if has_help {
            constraints.push(Constraint::Length(help_width));
        }
        constraints.push(Constraint::Min(min_board_width));
        if has_side {
            constraints.push(Constraint::Length(side_width));
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        let (help_panel, _board_area, side_panel) = if has_help && has_side {
            (Some(chunks[0]), chunks[1], Some(chunks[2]))
        } else if has_side {
            (None, chunks[0], Some(chunks[1]))
        } else {
            (None, chunks[0], None)
        };

        if let Some(help_area) = help_panel {
            self.render_help_panel(help_area, buf);
        }

        let mut board = BoardWidget::new(self.position)
            .selected(self.selected_square)
            .flipped(self.is_black_perspective)
            .highlights(
                self.highlight_origins.clone(),
                self.highlight_destinations.clone(),
            );

        if let Some((from, to)) = self.last_move {
            board = board.last_move(from, to);
        }

        board.render(Self::board_area(full_area), buf);

        if let Some(side_area) = side_panel {
            self.render_side_panel(side_area, buf);
        }
        if !has_side {
            let status = self
                .finished
                .as_deref()
                .or(self.status_message.as_deref())
                .unwrap_or("Click a piece or type a move");
            Paragraph::new(status)
                .style(Style::default().fg(ACCENT))
                .render(
                    Rect::new(area.x, area.y, area.width, area.height.min(1)),
                    buf,
                );
            if area.height > 0 {
                Paragraph::new(if self.finished.is_some() {
                    "ENTER: lobby | Q: quit".into()
                } else {
                    format!("> {}", self.input_buffer)
                })
                .render(Rect::new(area.x, area.bottom() - 1, area.width, 1), buf);
            }
        }
        for (control, rect) in GameControl::ALL
            .into_iter()
            .zip(controls::button_areas(full_area))
        {
            let enabled = match control {
                GameControl::Resign => self.finished.is_none(),
                GameControl::Draw => self.finished.is_none() && self.is_multiplayer,
                _ => true,
            };
            Paragraph::new(
                if control == GameControl::Draw
                    && self
                        .status_message
                        .as_deref()
                        .is_some_and(|s| s.contains("offers a draw"))
                    && full_area.width >= 78
                {
                    "F3 Accept draw"
                } else if control == GameControl::Disconnect && full_area.width >= 78 {
                    "F5 Disconnect"
                } else {
                    control.label()
                },
            )
            .style(Style::default().fg(if enabled { ACCENT } else { MUTED }))
            .block(panel(""))
            .render(rect, buf);
        }
    }
}

impl GameView<'_> {
    fn render_help_panel(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(vec![
            Line::from(Span::styled(
                " CheSSH",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )),
            Line::from(if self.finished.is_some() {
                " Final position"
            } else if self.computer {
                " Against Zander"
            } else if self.is_multiplayer {
                " Online game"
            } else {
                " Practice"
            }),
        ])
        .render(area, buf);
    }

    fn render_side_panel(&self, area: Rect, buf: &mut Buffer) {
        let side_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(5),
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(area);

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

        let is_top_turn = self.finished.is_none() && self.position.turn() == top_color;
        let is_bottom_turn = self.finished.is_none() && self.position.turn() == bottom_color;

        let top_style = if is_top_turn {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let top_player_widget = Paragraph::new(top_player.as_str())
            .style(top_style)
            .block(panel(if is_top_turn { " To move " } else { "" }));
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

        let moves_list = List::new(moves).block(panel(" Moves "));
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
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let bottom_player_widget = Paragraph::new(bottom_player.as_str())
            .style(bottom_style)
            .block(panel(if is_bottom_turn { " To move " } else { "" }));
        bottom_player_widget.render(side_chunks[4], buf);

        if let Some(result) = &self.finished {
            let color = if result.starts_with("YOU WIN") {
                Color::Green
            } else if result.starts_with("YOU LOSE") {
                Color::Red
            } else {
                Color::Yellow
            };
            Paragraph::new(result.as_str())
                .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
                .block(panel(" Result "))
                .render(side_chunks[5], buf);
            Paragraph::new("ENTER: lobby | Q: quit")
                .block(panel(" Review "))
                .render(side_chunks[6], buf);
            return;
        }

        if let Some(ref status) = self.status_message {
            let status_style = if status.contains("Check") {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if status.contains("Checkmate") || status.contains("wins") {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let status_widget = Paragraph::new(status.as_str())
                .style(status_style)
                .block(panel(" Status "));
            status_widget.render(side_chunks[5], buf);
        }

        let input_prefix = if self.is_my_turn { "> " } else { "· " };
        let input_text = format!("{}{}", input_prefix, self.input_buffer);
        let input_style = if self.is_my_turn {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let input_widget = Paragraph::new(input_text)
            .style(input_style)
            .block(panel(" Move "));
        input_widget.render(side_chunks[6], buf);
    }
}
