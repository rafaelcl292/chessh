use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use shakmaty::san::San;
use shakmaty::{Move, Position, Role, Square};

use crate::chess::Game;

use super::game_view::GameView;
use super::lobby::LobbyView;
use super::terminal::InputEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    Lobby,
    InQueue,
    Game,
    GameOver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameOverReason {
    YouWin(String),
    YouLose(String),
    Draw(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    None,
    JoinQueue,
    LeaveQueue,
    Quit,
    SubmitMove(Square, Square),
    SubmitMoveText(String),
    Resign,
    OfferDraw,
    ReturnToLobby,
}

pub struct App {
    view: AppView,
    input_buffer: String,
    online_count: usize,
    queue_size: usize,
    username: String,
    game: Option<Game>,
    selected_square: Option<Square>,
    highlight_origins: Vec<Square>,
    highlight_destinations: Vec<Square>,
    is_black_player: bool,
    is_multiplayer: bool,
    opponent_name: String,
    status_message: Option<String>,
    should_quit: bool,
    game_over_reason: Option<GameOverReason>,
}

impl App {
    pub fn new(username: String) -> Self {
        Self {
            view: AppView::Lobby,
            input_buffer: String::new(),
            online_count: 0,
            queue_size: 0,
            username,
            game: None,
            selected_square: None,
            highlight_origins: Vec::new(),
            highlight_destinations: Vec::new(),
            is_black_player: false,
            is_multiplayer: false,
            opponent_name: String::new(),
            status_message: None,
            should_quit: false,
            game_over_reason: None,
        }
    }

    pub fn is_my_turn(&self) -> bool {
        if let Some(game) = &self.game {
            let current_turn = game.turn();
            match current_turn {
                shakmaty::Color::White => !self.is_black_player,
                shakmaty::Color::Black => self.is_black_player,
            }
        } else {
            false
        }
    }

    pub fn is_multiplayer(&self) -> bool {
        self.is_multiplayer
    }

    pub fn view(&self) -> AppView {
        self.view
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn set_should_quit(&mut self, value: bool) {
        self.should_quit = value;
    }

    pub fn set_online_count(&mut self, count: usize) {
        self.online_count = count;
    }

    pub fn set_queue_size(&mut self, size: usize) {
        self.queue_size = size;
    }

    pub fn set_status(&mut self, message: Option<String>) {
        self.status_message = message;
    }

    pub fn join_queue(&mut self) {
        self.view = AppView::InQueue;
        self.status_message = Some("Searching for opponent...".to_string());
    }

    pub fn leave_queue(&mut self) {
        self.view = AppView::Lobby;
        self.status_message = None;
    }

    pub fn start_game(&mut self, opponent: String, is_black: bool) {
        self.start_game_with_mode(opponent, is_black, true)
    }

    pub fn start_solo_game(&mut self) {
        self.start_game_with_mode("Computer".to_string(), false, false)
    }

    fn start_game_with_mode(&mut self, opponent: String, is_black: bool, multiplayer: bool) {
        self.view = AppView::Game;
        self.game = Some(Game::new());
        self.opponent_name = opponent;
        self.is_black_player = is_black;
        self.is_multiplayer = multiplayer;
        self.selected_square = None;
        self.input_buffer.clear();
        self.status_message = None;
    }

    pub fn show_game_over(&mut self, reason: GameOverReason) {
        self.view = AppView::GameOver;
        self.game_over_reason = Some(reason);
        self.input_buffer.clear();
    }

    pub fn return_to_lobby(&mut self) {
        self.view = AppView::Lobby;
        self.game = None;
        self.selected_square = None;
        self.is_multiplayer = false;
        self.opponent_name.clear();
        self.input_buffer.clear();
        self.game_over_reason = None;
    }

    pub fn end_game(&mut self) {
        self.return_to_lobby();
    }

    pub fn game(&self) -> Option<&Game> {
        self.game.as_ref()
    }

    pub fn game_mut(&mut self) -> Option<&mut Game> {
        self.game.as_mut()
    }

    pub fn handle_input(&mut self, event: InputEvent) -> AppAction {
        match event {
            InputEvent::Key(KeyCode::Char('c'), modifiers)
                if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.should_quit = true;
                AppAction::Quit
            }
            InputEvent::Key(KeyCode::Char('d'), modifiers)
                if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.should_quit = true;
                AppAction::Quit
            }
            InputEvent::Key(KeyCode::Char(c), modifiers)
                if modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    && (c == '+' || c == '-' || c == '=' || c == '_' || c == '^') =>
            {
                AppAction::None
            }
            InputEvent::Key(key, _) => match self.view {
                AppView::Lobby => self.handle_lobby_input(key),
                AppView::InQueue => self.handle_queue_input(key),
                AppView::Game => self.handle_game_input(key),
                AppView::GameOver => self.handle_game_over_input(key),
            },
            _ => AppAction::None,
        }
    }

    fn handle_lobby_input(&mut self, key: KeyCode) -> AppAction {
        match key {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                AppAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                AppAction::None
            }
            KeyCode::Enter => {
                let cmd = self.input_buffer.trim().to_lowercase();
                self.input_buffer.clear();

                match cmd.as_str() {
                    "/play" | "play" | "/p" => AppAction::JoinQueue,
                    "/solo" | "solo" | "/s" => {
                        self.start_solo_game();
                        AppAction::None
                    }
                    "/quit" | "quit" | "/q" | "exit" => {
                        self.should_quit = true;
                        AppAction::Quit
                    }
                    _ => AppAction::None,
                }
            }
            KeyCode::Esc => {
                self.input_buffer.clear();
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    fn handle_queue_input(&mut self, key: KeyCode) -> AppAction {
        match key {
            KeyCode::Esc => AppAction::LeaveQueue,
            KeyCode::Char('q') => AppAction::LeaveQueue,
            _ => AppAction::None,
        }
    }

    fn handle_game_input(&mut self, key: KeyCode) -> AppAction {
        match key {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.try_parse_move()
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                if self.input_buffer.is_empty() {
                    self.clear_highlights();
                } else {
                    self.try_parse_move();
                }
                AppAction::None
            }
            KeyCode::Enter => {
                let input = self.input_buffer.trim().to_string();
                self.input_buffer.clear();

                if input.is_empty() {
                    return AppAction::None;
                }

                if input.starts_with('/') {
                    match input.to_lowercase().as_str() {
                        "/resign" => return AppAction::Resign,
                        "/draw" => return AppAction::OfferDraw,
                        "/quit" | "/q" => {
                            self.should_quit = true;
                            return AppAction::Quit;
                        }
                        "/back" | "/lobby" => {
                            return AppAction::Resign;
                        }
                        _ => {
                            self.status_message = Some("Unknown command".to_string());
                        }
                    }
                    self.clear_highlights();
                    return AppAction::None;
                }

                if let Some(game) = &self.game {
                    let matching_moves = self.find_matching_san_moves(game.position(), &input);

                    if matching_moves.len() == 1 {
                        let mv = matching_moves[0];
                        let san_str = San::from_move(game.position(), mv).to_string();

                        if self.is_multiplayer {
                            self.clear_highlights();
                            return AppAction::SubmitMoveText(san_str);
                        }

                        if let Some(game) = &mut self.game {
                            if game.play_move(mv).is_ok() {
                                self.clear_highlights();
                                self.status_message = None;
                                return AppAction::None;
                            }
                        }
                    }
                }

                if self.is_multiplayer {
                    self.clear_highlights();
                    return AppAction::SubmitMoveText(input);
                }

                if let Some(game) = &mut self.game {
                    let san_input = Self::normalize_san(&input);
                    if game.play_san(&san_input).is_ok()
                        || game.play_uci(&input.to_lowercase()).is_ok()
                    {
                        self.clear_highlights();
                        self.status_message = None;
                    } else {
                        self.status_message = Some(format!("Invalid move: {}", input));
                    }
                }
                self.clear_highlights();
                AppAction::None
            }
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.clear_highlights();
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    fn handle_game_over_input(&mut self, key: KeyCode) -> AppAction {
        match key {
            KeyCode::Enter => {
                self.return_to_lobby();
                AppAction::ReturnToLobby
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
                AppAction::Quit
            }
            _ => AppAction::None,
        }
    }

    fn try_parse_move(&mut self) -> AppAction {
        let input = self.input_buffer.trim();

        if input.is_empty() {
            self.clear_highlights();
            return AppAction::None;
        }

        if let Some(game) = &self.game {
            let matching_moves = self.find_matching_san_moves(game.position(), input);

            if !matching_moves.is_empty() {
                self.update_highlights_from_moves(&matching_moves);
            } else {
                let input_lower = input.to_lowercase();
                if input_lower.len() >= 2 {
                    if let Ok(sq) = input_lower[..2].parse::<Square>() {
                        self.selected_square = Some(sq);
                        self.highlight_origins.clear();
                        self.highlight_destinations.clear();

                        if input_lower.len() >= 4 {
                            if let Ok(to_sq) = input_lower[2..4].parse::<Square>() {
                                self.highlight_origins = vec![sq];
                                self.highlight_destinations = vec![to_sq];
                                if !self.is_multiplayer {
                                    return AppAction::SubmitMove(sq, to_sq);
                                }
                            }
                        }
                    } else {
                        self.clear_highlights();
                    }
                } else {
                    self.clear_highlights();
                }
            }
        }
        AppAction::None
    }

    fn find_matching_san_moves(&self, position: &shakmaty::Chess, input: &str) -> Vec<Move> {
        let normalized = Self::normalize_san(input);
        let legal_moves: Vec<Move> = position.legal_moves().into_iter().collect();

        if normalized.is_empty() {
            return Vec::new();
        }

        let first_char = normalized.chars().next().unwrap();

        if normalized == "O-O" || normalized == "O-O-O" {
            if let Ok(san) = normalized.parse::<San>() {
                return legal_moves
                    .into_iter()
                    .filter(|m| san.matches(*m))
                    .collect();
            }
            return Vec::new();
        }

        if first_char == 'O' || first_char == '0' {
            let partial = normalized.to_uppercase();
            return legal_moves
                .into_iter()
                .filter(|m| {
                    let san_str = San::from_move(position, *m).to_string();
                    san_str.starts_with(&partial)
                        || san_str
                            .replace('-', "")
                            .starts_with(&partial.replace('-', ""))
                })
                .collect();
        }

        let role = match first_char {
            'N' => Some(Role::Knight),
            'B' => Some(Role::Bishop),
            'R' => Some(Role::Rook),
            'Q' => Some(Role::Queen),
            'K' => Some(Role::King),
            _ => None,
        };

        if let Some(piece_role) = role {
            let rest = &normalized[1..];
            return legal_moves
                .into_iter()
                .filter(|m| {
                    if m.role() != piece_role {
                        return false;
                    }
                    if rest.is_empty() {
                        return true;
                    }
                    let san_str = San::from_move(position, *m).to_string();
                    san_str[1..].starts_with(rest)
                })
                .collect();
        }

        let first_lower = first_char.to_ascii_lowercase();
        if first_lower.is_ascii_lowercase() && ('a'..='h').contains(&first_lower) {
            return legal_moves
                .into_iter()
                .filter(|m| {
                    if m.role() != Role::Pawn {
                        return false;
                    }
                    let san_str = San::from_move(position, *m).to_string();
                    let san_lower = san_str.to_lowercase();
                    san_lower.starts_with(&normalized.to_lowercase())
                })
                .collect();
        }

        Vec::new()
    }

    fn update_highlights_from_moves(&mut self, moves: &[Move]) {
        self.highlight_origins.clear();
        self.highlight_destinations.clear();
        self.selected_square = None;

        for m in moves {
            if let Some(from) = m.from() {
                if !self.highlight_origins.contains(&from) {
                    self.highlight_origins.push(from);
                }
            }
            let to = m.to();
            if !self.highlight_destinations.contains(&to) {
                self.highlight_destinations.push(to);
            }
        }

        if self.highlight_origins.len() == 1 && self.highlight_destinations.len() == 1 {
            self.selected_square = Some(self.highlight_origins[0]);
        }
    }

    fn clear_highlights(&mut self) {
        self.selected_square = None;
        self.highlight_origins.clear();
        self.highlight_destinations.clear();
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

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        match self.view {
            AppView::Lobby | AppView::InQueue => self.render_lobby(area, buf),
            AppView::Game => self.render_game(area, buf),
            AppView::GameOver => self.render_game_over(area, buf),
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(self, area);

        let cursor_pos = self.get_cursor_position(area);
        frame.set_cursor_position(cursor_pos);
    }

    fn get_cursor_position(&self, area: Rect) -> (u16, u16) {
        match self.view {
            AppView::Lobby | AppView::InQueue => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(10), Constraint::Length(3)])
                    .split(area);

                let input_area = chunks[1];
                let cursor_x = input_area.x + 3 + self.input_buffer.len() as u16;
                let cursor_y = input_area.y + 1;
                (cursor_x.min(input_area.right() - 1), cursor_y)
            }
            AppView::Game => {
                if let Some(game) = &self.game {
                    let is_my_turn = self.is_my_turn();
                    let view = GameView::new(game.position(), game.san_history()).input(
                        self.input_buffer.clone(),
                        is_my_turn,
                        self.is_multiplayer,
                    );
                    view.get_input_position(area)
                } else {
                    (area.x, area.y)
                }
            }
            AppView::GameOver => (area.x + area.width / 2, area.y + area.height / 2 + 3),
        }
    }

    fn render_lobby(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(3)])
            .split(area);

        let lobby = LobbyView::new()
            .online_count(self.online_count)
            .queue_size(self.queue_size)
            .username(Some(self.username.clone()))
            .searching(self.view == AppView::InQueue);

        lobby.render(chunks[0], buf);

        let input_text = format!("> {}", self.input_buffer);
        let input = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Command"));
        input.render(chunks[1], buf);
    }

    fn render_game(&self, area: Rect, buf: &mut Buffer) {
        if let Some(game) = &self.game {
            let (white_name, black_name) = if self.is_black_player {
                (self.opponent_name.clone(), self.username.clone())
            } else {
                (self.username.clone(), self.opponent_name.clone())
            };

            let last_move = game
                .move_history()
                .last()
                .and_then(|m| Some((m.from()?, m.to())));

            let is_my_turn = self.is_my_turn();

            let mut view = GameView::new(game.position(), game.san_history())
                .players(white_name, black_name)
                .selected(self.selected_square)
                .highlights(
                    self.highlight_origins.clone(),
                    self.highlight_destinations.clone(),
                )
                .perspective(self.is_black_player)
                .input(self.input_buffer.clone(), is_my_turn, self.is_multiplayer);

            if let Some((from, to)) = last_move {
                view = view.last_move(from, to);
            }

            if let Some(status) = &self.status_message {
                view = view.status(status.clone());
            } else if game.is_check() {
                view = view.status("Check!".to_string());
            } else if game.is_checkmate() {
                let winner = if game.turn() == shakmaty::Color::White {
                    "Black wins!"
                } else {
                    "White wins!"
                };
                view = view.status(format!("Checkmate! {}", winner));
            } else if game.is_stalemate() {
                view = view.status("Stalemate! Draw.".to_string());
            }

            view.render(area, buf);
        }
    }

    fn render_game_over(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::layout::Alignment;
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};

        let (title, reason, title_color) = match &self.game_over_reason {
            Some(GameOverReason::YouWin(reason)) => {
                ("🎉 YOU WIN! 🎉".to_string(), reason.clone(), Color::Green)
            }
            Some(GameOverReason::YouLose(reason)) => {
                ("YOU LOSE".to_string(), reason.clone(), Color::Red)
            }
            Some(GameOverReason::Draw(reason)) => {
                ("DRAW".to_string(), reason.clone(), Color::Yellow)
            }
            None => ("Game Over".to_string(), String::new(), Color::White),
        };

        let lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "♔ Game Over ♚",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                title,
                Style::default()
                    .fg(title_color)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(reason, Style::default().fg(title_color))),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press ENTER to return to lobby",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Q to quit",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        paragraph.render(area, buf);
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render(area, buf);
    }
}
