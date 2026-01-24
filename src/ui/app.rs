use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;
use shakmaty::Square;

use crate::chess::Game;

use super::game_view::GameView;
use super::lobby::LobbyView;
use super::terminal::InputEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    Lobby,
    InQueue,
    Game,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    None,
    JoinQueue,
    LeaveQueue,
    Quit,
    SubmitMove(Square, Square),
    Resign,
    OfferDraw,
}

pub struct App {
    view: AppView,
    input_buffer: String,
    online_count: usize,
    queue_size: usize,
    username: String,
    game: Option<Game>,
    selected_square: Option<Square>,
    is_black_player: bool,
    opponent_name: String,
    status_message: Option<String>,
    should_quit: bool,
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
            is_black_player: false,
            opponent_name: String::new(),
            status_message: None,
            should_quit: false,
        }
    }

    pub fn view(&self) -> AppView {
        self.view
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
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
        self.view = AppView::Game;
        self.game = Some(Game::new());
        self.opponent_name = opponent;
        self.is_black_player = is_black;
        self.selected_square = None;
        self.input_buffer.clear();
        self.status_message = None;
    }

    pub fn end_game(&mut self) {
        self.view = AppView::Lobby;
        self.game = None;
        self.selected_square = None;
        self.opponent_name.clear();
        self.input_buffer.clear();
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
            InputEvent::Key(key, _) => match self.view {
                AppView::Lobby => self.handle_lobby_input(key),
                AppView::InQueue => self.handle_queue_input(key),
                AppView::Game => self.handle_game_input(key),
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
                        self.start_game("Computer".to_string(), false);
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
                self.selected_square = None;
                AppAction::None
            }
            KeyCode::Enter => {
                let input = self.input_buffer.trim().to_string();
                self.input_buffer.clear();

                if input.starts_with('/') {
                    match input.to_lowercase().as_str() {
                        "/resign" => return AppAction::Resign,
                        "/draw" => return AppAction::OfferDraw,
                        "/quit" | "/q" => {
                            self.should_quit = true;
                            return AppAction::Quit;
                        }
                        "/back" | "/lobby" => {
                            self.end_game();
                            return AppAction::None;
                        }
                        _ => {
                            self.status_message = Some("Unknown command".to_string());
                        }
                    }
                } else if let Some(game) = &mut self.game {
                    let san_input = Self::normalize_san(&input);
                    if game.play_san(&san_input).is_ok() {
                        self.selected_square = None;
                        self.status_message = None;
                    } else if game.play_uci(&input.to_lowercase()).is_ok() {
                        self.selected_square = None;
                        self.status_message = None;
                    } else {
                        self.status_message = Some(format!("Invalid move: {}", input));
                    }
                }
                self.selected_square = None;
                AppAction::None
            }
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.selected_square = None;
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    fn try_parse_move(&mut self) -> AppAction {
        let input = self.input_buffer.trim().to_lowercase();

        if input.len() >= 2 {
            if let Ok(sq) = input[..2].parse::<Square>() {
                if input.len() == 2 {
                    self.selected_square = Some(sq);
                } else if input.len() >= 4 {
                    if let Ok(to_sq) = input[2..4].parse::<Square>() {
                        return AppAction::SubmitMove(sq, to_sq);
                    }
                }
            }
        }
        AppAction::None
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
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(20), Constraint::Length(3)])
                    .split(area);

                let input_area = chunks[1];
                let prefix_len = if self
                    .game
                    .as_ref()
                    .map(|g| {
                        (g.turn() == shakmaty::Color::White && !self.is_black_player)
                            || (g.turn() == shakmaty::Color::Black && self.is_black_player)
                    })
                    .unwrap_or(false)
                {
                    "Your move: ".len()
                } else {
                    "Waiting for opponent: ".len()
                };
                let cursor_x =
                    input_area.x + 1 + prefix_len as u16 + self.input_buffer.len() as u16;
                let cursor_y = input_area.y + 1;
                (cursor_x.min(input_area.right() - 1), cursor_y)
            }
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
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(20), Constraint::Length(3)])
                .split(area);

            let (white_name, black_name) = if self.is_black_player {
                (self.opponent_name.clone(), self.username.clone())
            } else {
                (self.username.clone(), self.opponent_name.clone())
            };

            let last_move = game
                .move_history()
                .last()
                .and_then(|m| Some((m.from()?, m.to())));

            let mut view = GameView::new(game.position(), game.san_history())
                .players(white_name, black_name)
                .selected(self.selected_square)
                .perspective(self.is_black_player);

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

            view.render(chunks[0], buf);

            let input_prompt = if game.turn() == shakmaty::Color::White && !self.is_black_player
                || game.turn() == shakmaty::Color::Black && self.is_black_player
            {
                "Your move"
            } else {
                "Waiting for opponent"
            };

            let input_text = format!("{}: {}", input_prompt, self.input_buffer);
            let input = Paragraph::new(input_text)
                .block(Block::default().borders(Borders::ALL).title("Move"));
            input.render(chunks[1], buf);
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render(area, buf);
    }
}
