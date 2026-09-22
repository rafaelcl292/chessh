use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui::Frame;
use shakmaty::san::San;
use shakmaty::{Move, Position, Role, Square};

use crate::chess::{Game, GameResult};

use super::controls::{self, GameControl};
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
    StartComputer(u8),
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
    area: Rect,
    promotion: Vec<Move>,
    confirmation: Option<GameControl>,
    confirm_selected: bool,
    draw_received: bool,
    lobby_selection: usize,
    lobby_help: bool,
    computer_setup: bool,
    engine_level: u8,
    computer: bool,
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
            area: Rect::default(),
            promotion: Vec::new(),
            confirmation: None,
            confirm_selected: false,
            draw_received: false,
            lobby_selection: 0,
            lobby_help: false,
            computer_setup: false,
            engine_level: 5,
            computer: false,
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

    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
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

    pub fn is_computer(&self) -> bool {
        self.computer
    }

    pub fn start_computer_game(&mut self, level: u8) {
        self.start_game_with_mode(format!("Zander · Level {level}"), false, false);
        self.computer = true;
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
        self.start_game_with_mode("Practice".to_string(), false, false)
    }

    fn start_game_with_mode(&mut self, opponent: String, is_black: bool, multiplayer: bool) {
        self.confirmation = None;
        self.draw_received = false;
        self.view = AppView::Game;
        self.game = Some(Game::new());
        self.opponent_name = opponent;
        self.is_black_player = is_black;
        self.is_multiplayer = multiplayer;
        self.computer = false;
        self.computer_setup = false;
        self.clear_highlights();
        self.game_over_reason = None;
        self.input_buffer.clear();
        self.status_message = None;
    }

    pub fn update_game(&mut self, game: Game) {
        self.draw_received = false;
        if self.confirmation == Some(GameControl::Draw) {
            self.confirmation = None;
        }
        self.game = Some(game);
        self.status_message = None;
        self.clear_highlights();
    }

    pub fn finish_game(&mut self, reason: String) {
        let Some(game) = &self.game else {
            return;
        };
        let winner = match game.result() {
            GameResult::WhiteWins | GameResult::BlackResigned => Some(shakmaty::Color::White),
            GameResult::BlackWins | GameResult::WhiteResigned => Some(shakmaty::Color::Black),
            GameResult::Draw => None,
            GameResult::Ongoing => return,
        };
        let my_color = if self.is_black_player {
            shakmaty::Color::Black
        } else {
            shakmaty::Color::White
        };
        let result = match winner {
            Some(color) if color == my_color => GameOverReason::YouWin(reason),
            Some(_) => GameOverReason::YouLose(reason),
            None => GameOverReason::Draw(reason),
        };
        self.show_game_over(result);
    }

    pub fn show_game_over(&mut self, reason: GameOverReason) {
        self.confirmation = None;
        self.view = AppView::GameOver;
        self.game_over_reason = Some(reason);
        self.input_buffer.clear();
        self.clear_highlights();
        self.status_message = None;
    }

    pub fn return_to_lobby(&mut self) {
        self.confirmation = None;
        self.draw_received = false;
        self.view = AppView::Lobby;
        self.game = None;
        self.selected_square = None;
        self.is_multiplayer = false;
        self.computer = false;
        self.opponent_name.clear();
        self.input_buffer.clear();
        self.game_over_reason = None;
        self.status_message = None;
        self.clear_highlights();
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
        if matches!(event, InputEvent::Key(KeyCode::Char('c' | 'd'), modifiers)
            if modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
        {
            self.should_quit = true;
            return AppAction::Quit;
        }
        if self.confirmation.is_some() {
            return self.handle_confirmation(event);
        }
        if matches!(self.view, AppView::Game | AppView::GameOver) {
            if let InputEvent::Key(KeyCode::F(number @ 2..=5), _) = event {
                return self.request_control(GameControl::ALL[(number - 2) as usize]);
            }
        }
        match event {
            InputEvent::Click(x, y) => self.handle_click(x, y),
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

    pub fn receive_draw_offer(&mut self) {
        // A changed offer must not turn an already-open "offer" dialog into acceptance.
        if self.confirmation == Some(GameControl::Draw) {
            self.confirmation = None;
        }
        self.draw_received = true;
        self.status_message = Some("Opponent offers a draw. F3 to accept".into());
    }

    fn request_control(&mut self, control: GameControl) -> AppAction {
        if self.view == AppView::GameOver {
            return match control {
                GameControl::Lobby => {
                    self.return_to_lobby();
                    AppAction::ReturnToLobby
                }
                GameControl::Disconnect => {
                    self.should_quit = true;
                    AppAction::Quit
                }
                _ => AppAction::None,
            };
        }
        if control == GameControl::Draw && !self.is_multiplayer {
            self.status_message = Some("Draw offers are available in online games.".into());
            return AppAction::None;
        }
        if !self.is_multiplayer && matches!(control, GameControl::Lobby | GameControl::Disconnect) {
            return self.execute_control(control);
        }
        self.confirmation = Some(control);
        self.confirm_selected = false;
        AppAction::None
    }

    fn execute_control(&mut self, control: GameControl) -> AppAction {
        self.confirmation = None;
        match control {
            GameControl::Resign => AppAction::Resign,
            GameControl::Draw => AppAction::OfferDraw,
            GameControl::Lobby => AppAction::ReturnToLobby,
            GameControl::Disconnect => {
                self.should_quit = true;
                AppAction::Quit
            }
        }
    }

    fn handle_confirmation(&mut self, event: InputEvent) -> AppAction {
        let control = self.confirmation.unwrap();
        match event {
            InputEvent::Key(KeyCode::Esc | KeyCode::Char('n'), _) => self.confirmation = None,
            InputEvent::Key(
                KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Char('h' | 'l'),
                _,
            ) => self.confirm_selected = !self.confirm_selected,
            InputEvent::Key(KeyCode::Char('y'), _) => return self.execute_control(control),
            InputEvent::Key(KeyCode::Enter, _) => {
                if self.confirm_selected {
                    return self.execute_control(control);
                }
                self.confirmation = None;
            }
            InputEvent::Click(x, y) => {
                let choices = controls::choices(self.area);
                if choices[0].contains((x, y).into()) {
                    self.confirmation = None;
                } else if choices[1].contains((x, y).into()) {
                    return self.execute_control(control);
                }
            }
            _ => {}
        }
        AppAction::None
    }

    fn confirmation_message(&self) -> &'static str {
        match self.confirmation {
            Some(GameControl::Resign) => "Resign this game? This ends the game as a loss.",
            Some(GameControl::Draw) if self.draw_received => {
                "Accept your opponent's draw offer? This ends the game in a draw."
            }
            Some(GameControl::Draw) => {
                "Offer a draw? The game continues until your opponent accepts."
            }
            Some(GameControl::Lobby) => "Return to the lobby? You will resign the current game.",
            Some(GameControl::Disconnect) => {
                "Disconnect? Leaving now awards the game to your opponent."
            }
            None => "",
        }
    }

    fn promotion_area(&self) -> Rect {
        let width = self.area.width.min(26);
        let height = self.area.height.min(7);
        Rect::new(
            self.area.x + (self.area.width - width) / 2,
            self.area.y + (self.area.height - height) / 2,
            width,
            height,
        )
    }

    fn play_clicked_move(&mut self, mv: Move) -> AppAction {
        let text = mv.to_uci(shakmaty::CastlingMode::Standard).to_string();
        self.input_buffer.clear();
        self.clear_highlights();
        AppAction::SubmitMoveText(text)
    }

    fn handle_click(&mut self, x: u16, y: u16) -> AppAction {
        if matches!(self.view, AppView::Game | AppView::GameOver) && self.promotion.is_empty() {
            for (control, rect) in GameControl::ALL
                .into_iter()
                .zip(controls::button_areas(self.area))
            {
                if rect.contains((x, y).into()) {
                    return self.request_control(control);
                }
            }
        }
        match self.view {
            AppView::Lobby => {
                if self.computer_setup {
                    let inner = LobbyView::menu_inner(self.area);
                    if inner.contains((x, y).into()) {
                        match y.saturating_sub(inner.y) {
                            4 => {
                                if x < inner.x + inner.width / 2 {
                                    self.engine_level = self.engine_level.saturating_sub(1);
                                } else {
                                    self.engine_level = (self.engine_level + 1).min(20);
                                }
                            }
                            6 => return AppAction::StartComputer(self.engine_level),
                            8 => self.computer_setup = false,
                            _ => {}
                        }
                    }
                } else if self.lobby_help {
                    let inner = LobbyView::menu_inner(self.area);
                    if inner.contains((x, y).into()) && y == inner.bottom().saturating_sub(1) {
                        self.lobby_help = false;
                    }
                } else if let Some(index) = LobbyView::option_at(self.area, x, y) {
                    self.input_buffer.clear();
                    self.lobby_selection = index;
                    return self.activate_lobby_selection();
                }
            }
            AppView::InQueue => {
                let inner = LobbyView::menu_inner(self.area);
                if inner.contains((x, y).into()) && y == inner.bottom().saturating_sub(1) {
                    return AppAction::LeaveQueue;
                }
            }
            AppView::Game => {
                if (self.is_multiplayer || self.computer) && !self.is_my_turn() {
                    return AppAction::None;
                }
                if !self.promotion.is_empty() {
                    let area = self.promotion_area();
                    if area.contains((x, y).into()) {
                        let role = match y.saturating_sub(area.y) {
                            1 => Some(Role::Queen),
                            2 => Some(Role::Rook),
                            3 => Some(Role::Bishop),
                            4 => Some(Role::Knight),
                            _ => None,
                        };
                        if let Some(mv) = self
                            .promotion
                            .iter()
                            .copied()
                            .find(|m| m.promotion() == role)
                        {
                            return self.play_clicked_move(mv);
                        }
                    }
                    return AppAction::None;
                }
                let Some(square) = super::board::BoardWidget::square_at(
                    GameView::board_area(self.area),
                    x,
                    y,
                    self.is_black_player,
                ) else {
                    return AppAction::None;
                };
                let Some(game) = &self.game else {
                    return AppAction::None;
                };
                if self.selected_square == Some(square) {
                    self.clear_highlights();
                    return AppAction::None;
                }
                let moves: Vec<_> = game.position().legal_moves().into_iter().collect();
                if let Some(from) = self.selected_square {
                    let candidates: Vec<_> = moves
                        .iter()
                        .copied()
                        .filter(|mv| {
                            let uci = mv.to_uci(shakmaty::CastlingMode::Standard);
                            uci.from() == Some(from) && uci.to() == Some(square)
                        })
                        .collect();
                    if candidates.len() == 1 {
                        return self.play_clicked_move(candidates[0]);
                    }
                    if candidates.len() > 1 {
                        self.promotion = candidates;
                        return AppAction::None;
                    }
                }
                if game
                    .position()
                    .board()
                    .piece_at(square)
                    .is_some_and(|piece| piece.color == game.turn())
                {
                    self.input_buffer.clear();
                    self.selected_square = Some(square);
                    self.highlight_origins.clear();
                    self.highlight_destinations = moves
                        .iter()
                        .filter(|mv| mv.from() == Some(square))
                        .filter_map(|mv| mv.to_uci(shakmaty::CastlingMode::Standard).to())
                        .collect();
                } else {
                    self.clear_highlights();
                }
            }
            AppView::GameOver => {} // Keep the final position available for review.
        }
        AppAction::None
    }

    fn handle_lobby_input(&mut self, key: KeyCode) -> AppAction {
        if self.computer_setup {
            match key {
                KeyCode::Left | KeyCode::Down | KeyCode::Char('h' | 'j') => {
                    self.engine_level = self.engine_level.saturating_sub(1)
                }
                KeyCode::Right | KeyCode::Up | KeyCode::Char('l' | 'k') => {
                    self.engine_level = (self.engine_level + 1).min(20)
                }
                KeyCode::Enter => return AppAction::StartComputer(self.engine_level),
                KeyCode::Esc => self.computer_setup = false,
                _ => {}
            }
            return AppAction::None;
        }
        if self.lobby_help {
            if matches!(
                key,
                KeyCode::Enter | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h')
            ) {
                self.lobby_help = false;
            }
            return AppAction::None;
        }
        if self.input_buffer.is_empty() {
            match key {
                KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
                    self.lobby_selection = (self.lobby_selection + 1) % 5;
                    return AppAction::None;
                }
                KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                    self.lobby_selection = (self.lobby_selection + 4) % 5;
                    return AppAction::None;
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    return self.activate_lobby_selection()
                }
                KeyCode::Char(c @ '1'..='5') => {
                    self.lobby_selection = (c as u8 - b'1') as usize;
                    return self.activate_lobby_selection();
                }
                KeyCode::Char('/') => {}
                _ => return AppAction::None,
            }
        }
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
                    "/computer" | "/ai" => {
                        self.computer_setup = true;
                        AppAction::None
                    }
                    "/play" | "play" | "/p" => AppAction::JoinQueue,
                    "/solo" | "solo" | "/s" => {
                        self.start_solo_game();
                        AppAction::None
                    }
                    "/quit" | "quit" | "/q" | "exit" => {
                        self.should_quit = true;
                        AppAction::Quit
                    }
                    _ => {
                        self.status_message =
                            Some("Unknown command. Use /play, /solo, /ai, /quit.".into());
                        AppAction::None
                    }
                }
            }
            KeyCode::Esc => {
                self.input_buffer.clear();
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    fn activate_lobby_selection(&mut self) -> AppAction {
        self.status_message = None;
        match self.lobby_selection {
            0 => AppAction::JoinQueue,
            1 => {
                self.start_solo_game();
                AppAction::None
            }
            2 => {
                self.computer_setup = true;
                AppAction::None
            }
            3 => {
                self.lobby_help = true;
                AppAction::None
            }
            _ => {
                self.should_quit = true;
                AppAction::Quit
            }
        }
    }

    fn handle_queue_input(&mut self, key: KeyCode) -> AppAction {
        match key {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Left | KeyCode::Char('h') => {
                AppAction::LeaveQueue
            }
            KeyCode::Char('q') => AppAction::LeaveQueue,
            _ => AppAction::None,
        }
    }

    fn handle_game_input(&mut self, key: KeyCode) -> AppAction {
        if !self.promotion.is_empty() {
            if key == KeyCode::Esc {
                self.clear_highlights();
                return AppAction::None;
            }
            let role = match key {
                KeyCode::Char('q') => Some(Role::Queen),
                KeyCode::Char('r') => Some(Role::Rook),
                KeyCode::Char('b') => Some(Role::Bishop),
                KeyCode::Char('n') => Some(Role::Knight),
                _ => None,
            };
            if let Some(mv) = self
                .promotion
                .iter()
                .copied()
                .find(|m| m.promotion() == role)
            {
                return self.play_clicked_move(mv);
            }
            return AppAction::None;
        }
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
                        "/resign" => return self.request_control(GameControl::Resign),
                        "/draw" => return self.request_control(GameControl::Draw),
                        "/quit" | "/q" => return self.request_control(GameControl::Disconnect),
                        "/back" | "/lobby" => return self.request_control(GameControl::Lobby),
                        _ => {
                            self.status_message = Some("Unknown command".to_string());
                        }
                    }
                    self.clear_highlights();
                    return AppAction::None;
                }

                if self.computer && !self.is_my_turn() {
                    self.set_status(Some("Zander is thinking...".into()));
                    return AppAction::None;
                }
                if let Some(game) = &self.game {
                    let matching_moves = self.find_matching_san_moves(game.position(), &input);

                    if matching_moves.len() == 1 {
                        let mv = matching_moves[0];
                        let san_str = San::from_move(game.position(), mv).to_string();

                        if self.is_multiplayer || self.computer {
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

                if self.is_multiplayer || self.computer {
                    self.clear_highlights();
                    return AppAction::SubmitMoveText(input);
                }

                if let Some(game) = &mut self.game {
                    let san_input = Self::normalize_san(&input);
                    if game.play_san(&input).is_ok()
                        || game.play_san(&san_input).is_ok()
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
                    if let Ok(sq) = input_lower.get(..2).unwrap_or("").parse::<Square>() {
                        self.selected_square = Some(sq);
                        self.highlight_origins.clear();
                        self.highlight_destinations.clear();

                        if input_lower.len() >= 4 {
                            if let Ok(to_sq) = input_lower.get(2..4).unwrap_or("").parse::<Square>()
                            {
                                self.highlight_origins = vec![sq];
                                self.highlight_destinations = vec![to_sq];
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
        // Preserve valid pawn SAN on the b-file before accepting lowercase piece shortcuts.
        if let Ok(san) = input.parse::<San>() {
            if let Ok(mv) = san.to_move(position) {
                return vec![mv];
            }
        }
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
        self.promotion.clear();
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
            AppView::Game | AppView::GameOver => self.render_game(area, buf),
        }
        if !self.promotion.is_empty() {
            let popup = self.promotion_area();
            Clear.render(popup, buf);
            Paragraph::new("Q  Queen\nR  Rook\nB  Bishop\nN  Knight\nEsc  Cancel")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Promote pawn "),
                )
                .render(popup, buf);
        }
        if self.confirmation.is_some() {
            controls::confirmation(
                area,
                buf,
                self.confirmation_message(),
                self.confirm_selected,
            );
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(self, area);

        if self.view == AppView::Game && self.confirmation.is_none() && self.promotion.is_empty() {
            let cursor_pos = self.get_cursor_position(area);
            frame.set_cursor_position(cursor_pos);
        }
    }

    fn get_cursor_position(&self, area: Rect) -> (u16, u16) {
        match self.view {
            AppView::Lobby | AppView::InQueue => (area.x, area.y),
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
        LobbyView {
            online_count: self.online_count,
            queue_size: self.queue_size,
            username: &self.username,
            searching: self.view == AppView::InQueue,
            selected: self.lobby_selection,
            help: self.lobby_help,
            computer_level: self.computer_setup.then_some(self.engine_level),
            command: &self.input_buffer,
            status: self.status_message.as_deref(),
        }
        .render(area, buf);
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
                .computer(self.computer)
                .perspective(self.is_black_player)
                .input(self.input_buffer.clone(), is_my_turn, self.is_multiplayer);

            if let Some((from, to)) = last_move {
                view = view.last_move(from, to);
            }

            if let Some(reason) = &self.game_over_reason {
                let message = match reason {
                    GameOverReason::YouWin(reason) => format!("YOU WIN - {reason}"),
                    GameOverReason::YouLose(reason) => format!("YOU LOSE - {reason}"),
                    GameOverReason::Draw(reason) => format!("DRAW - {reason}"),
                };
                view = view.finished(message);
            } else if let Some(status) = &self.status_message {
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
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(app: &mut App, code: KeyCode) -> AppAction {
        app.handle_input(InputEvent::Key(code, KeyModifiers::NONE))
    }

    #[test]
    fn computer_level_selection_clamps_and_guards_engine_turn() {
        let mut app = App::new("player".into());
        key(&mut app, KeyCode::Char('3'));
        assert!(app.computer_setup);
        for _ in 0..30 {
            key(&mut app, KeyCode::Left);
        }
        assert_eq!(key(&mut app, KeyCode::Enter), AppAction::StartComputer(0));
        for _ in 0..30 {
            key(&mut app, KeyCode::Right);
        }
        assert_eq!(key(&mut app, KeyCode::Enter), AppAction::StartComputer(20));
        app.start_computer_game(20);
        app.game_mut().unwrap().play_uci("e2e4").unwrap();
        for c in "e5".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(key(&mut app, KeyCode::Enter), AppAction::None);
        assert_eq!(app.game().unwrap().move_count(), 1);
        assert_eq!(key(&mut app, KeyCode::F(4)), AppAction::ReturnToLobby);
        app.return_to_lobby();
        assert!(!app.is_computer());
    }

    #[test]
    fn computer_level_mouse_controls_match_rendered_rows() {
        let mut app = App::new("player".into());
        app.set_area(Rect::new(0, 0, 80, 24));
        key(&mut app, KeyCode::Char('3'));
        let inner = LobbyView::menu_inner(app.area);
        app.handle_input(InputEvent::Click(inner.right() - 1, inner.y + 4));
        assert_eq!(app.engine_level, 6);
        assert_eq!(
            app.handle_input(InputEvent::Click(inner.x + 2, inner.y + 6)),
            AppAction::StartComputer(6)
        );
    }

    #[test]
    fn lobby_keyboard_navigation_help_practice_and_queue() {
        let mut app = App::new("player".into());
        assert_eq!(key(&mut app, KeyCode::Enter), AppAction::JoinQueue);
        app.join_queue();
        assert_eq!(key(&mut app, KeyCode::Enter), AppAction::LeaveQueue);
        app.leave_queue();
        key(&mut app, KeyCode::Up);
        assert_eq!(app.lobby_selection, 4);
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Char('j'));
        key(&mut app, KeyCode::Char('l'));
        assert_eq!(app.view(), AppView::Game);
        assert!(!app.is_multiplayer());
        app.return_to_lobby();
        key(&mut app, KeyCode::Char('4'));
        assert!(app.lobby_help);
        key(&mut app, KeyCode::Char('1'));
        assert_eq!(app.view(), AppView::Lobby);
        key(&mut app, KeyCode::Char('h'));
        assert!(!app.lobby_help);
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(app.lobby_selection, 2);
        for c in "/solo".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.view(), AppView::Game);
    }

    #[test]
    fn lobby_is_usable_at_common_sizes_and_safe_at_tiny_sizes() {
        let mut app = App::new("player".into());
        for (width, height) in [(100, 30), (80, 24), (44, 24), (40, 16)] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
            app.render(buffer.area, &mut buffer);
            let rendered = text(&buffer);
            for label in [
                "Play online",
                "Practice board",
                "Play computer",
                "How to play",
                "Disconnect",
                "Enter",
            ] {
                assert!(
                    rendered.contains(label),
                    "{width}x{height}: missing {label}"
                );
            }
        }
        for (width, height) in [(0, 0), (1, 1), (20, 8)] {
            for view in [AppView::Lobby, AppView::InQueue] {
                app.view = view;
                let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
                app.render(buffer.area, &mut buffer);
            }
        }
    }

    fn click_square(app: &mut App, square: Square) -> AppAction {
        let board = GameView::board_area(app.area);
        for y in board.y..board.bottom() {
            for x in board.x..board.right() {
                if super::super::board::BoardWidget::square_at(board, x, y, app.is_black_player)
                    == Some(square)
                {
                    return app.handle_input(InputEvent::Click(x, y));
                }
            }
        }
        panic!("square not rendered");
    }

    #[test]
    fn mouse_selects_legal_moves_in_both_board_sizes_and_orientations() {
        for area in [
            Rect::new(0, 0, 100, 30),
            Rect::new(0, 0, 190, 50),
            Rect::new(0, 0, 44, 24),
        ] {
            for flipped in [false, true] {
                let mut app = App::new("player".into());
                app.set_area(area);
                app.start_solo_game();
                app.is_black_player = flipped;
                assert_eq!(click_square(&mut app, Square::E2), AppAction::None);
                assert!(app.highlight_destinations.contains(&Square::E4));
                assert_eq!(
                    click_square(&mut app, Square::E4),
                    AppAction::SubmitMoveText("e2e4".into())
                );
                assert_eq!(app.handle_input(InputEvent::Click(0, 0)), AppAction::None);
            }
        }
    }

    #[test]
    fn mouse_supports_castling_promotion_and_turn_guards() {
        let mut app = App::new("player".into());
        app.set_area(Rect::new(0, 0, 100, 30));
        app.start_solo_game();
        app.update_game(Game::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1").unwrap());
        click_square(&mut app, Square::E1);
        assert_eq!(
            click_square(&mut app, Square::G1),
            AppAction::SubmitMoveText("e1g1".into())
        );
        app.update_game(Game::from_fen("7k/P7/8/8/8/8/8/7K w - - 0 1").unwrap());
        click_square(&mut app, Square::A7);
        click_square(&mut app, Square::A8);
        assert_eq!(app.promotion.len(), 4);
        let popup = app.promotion_area();
        assert_eq!(
            app.handle_input(InputEvent::Click(popup.x + 2, popup.y + 4)),
            AppAction::SubmitMoveText("a7a8n".into())
        );
        app.start_game("opponent".into(), true);
        click_square(&mut app, Square::E2);
        assert!(app.selected_square.is_none());
        app.show_game_over(GameOverReason::Draw("test".into()));
        assert_eq!(click_square(&mut app, Square::E2), AppAction::None);
        assert_eq!(app.view(), AppView::GameOver);
    }

    #[test]
    fn mouse_opens_menu_and_cancels_search_at_different_sizes() {
        for area in [Rect::new(0, 0, 100, 30), Rect::new(0, 0, 44, 24)] {
            let mut app = App::new("player".into());
            app.set_area(area);
            let inner = LobbyView::menu_inner(area);
            assert_eq!(
                app.handle_input(InputEvent::Click(inner.x + 2, inner.y + 2)),
                AppAction::JoinQueue
            );
            app.join_queue();
            assert_eq!(
                app.handle_input(InputEvent::Click(inner.x + 2, inner.bottom() - 1)),
                AppAction::LeaveQueue
            );
        }
    }

    #[test]
    fn game_controls_require_confirmation_and_preserve_move_input() {
        let mut app = App::new("player".into());
        app.set_area(Rect::new(0, 0, 100, 30));
        app.start_game("opponent".into(), false);
        key(&mut app, KeyCode::Char('e'));
        key(&mut app, KeyCode::F(2));
        assert_eq!(app.confirmation, Some(GameControl::Resign));
        assert_eq!(key(&mut app, KeyCode::Enter), AppAction::None);
        assert!(app.confirmation.is_none());
        assert_eq!(app.input_buffer, "e");
        key(&mut app, KeyCode::F(2));
        assert_eq!(key(&mut app, KeyCode::Char('y')), AppAction::Resign);
        for (number, expected) in [
            (3, AppAction::OfferDraw),
            (4, AppAction::ReturnToLobby),
            (5, AppAction::Quit),
        ] {
            key(&mut app, KeyCode::F(number));
            assert!(!app.should_quit());
            assert_eq!(key(&mut app, KeyCode::Char('y')), expected);
        }
    }

    #[test]
    fn clickable_controls_and_modal_work_in_wide_and_compact_layouts() {
        for area in [Rect::new(0, 0, 190, 50), Rect::new(0, 0, 44, 24)] {
            let mut app = App::new("player".into());
            app.set_area(area);
            app.start_game("opponent".into(), false);
            let button = controls::button_areas(area)[0];
            assert_eq!(
                app.handle_input(InputEvent::Click(button.x + 1, button.y + 1)),
                AppAction::None
            );
            let mut buffer = Buffer::empty(area);
            app.render(area, &mut buffer);
            assert!(text(&buffer).contains("Confirm action"));
            let cancel = controls::choices(area)[0];
            app.handle_input(InputEvent::Click(cancel.x + 1, cancel.y + 1));
            assert!(app.confirmation.is_none());
            key(&mut app, KeyCode::F(2));
            let confirm = controls::choices(area)[1];
            assert_eq!(
                app.handle_input(InputEvent::Click(confirm.x + 1, confirm.y + 1)),
                AppAction::Resign
            );
        }
    }

    #[test]
    fn changed_game_state_invalidates_stale_draw_confirmations() {
        let mut app = App::new("player".into());
        app.start_game("opponent".into(), false);
        key(&mut app, KeyCode::F(3));
        app.receive_draw_offer();
        assert!(app.confirmation.is_none());
        key(&mut app, KeyCode::F(3));
        assert!(app.confirmation_message().starts_with("Accept"));
        app.update_game(Game::new());
        assert!(app.confirmation.is_none());
        key(&mut app, KeyCode::F(2));
        app.show_game_over(GameOverReason::Draw("Game over".into()));
        assert!(app.confirmation.is_none());
        app.start_solo_game();
        key(&mut app, KeyCode::F(3));
        assert!(app.confirmation.is_none());
    }

    fn mate(white_wins: bool) -> Game {
        let mut game = Game::new();
        let moves = if white_wins {
            vec!["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7#"]
        } else {
            vec!["f3", "e5", "g4", "Qh4#"]
        };
        for mv in moves {
            game.play_san(mv).unwrap();
        }
        game
    }

    fn text(buf: &Buffer) -> String {
        buf.content.iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn winner_and_loser_are_correct_for_both_colors() {
        for white_wins in [false, true] {
            for is_black in [false, true] {
                let mut app = App::new("me".into());
                app.start_game("opponent".into(), is_black);
                app.update_game(mate(white_wins));
                app.finish_game("Checkmate".into());
                assert_eq!(
                    matches!(app.game_over_reason, Some(GameOverReason::YouWin(_))),
                    white_wins != is_black
                );
            }
        }
    }

    #[test]
    fn final_board_stays_visible_and_unchanged_in_both_orientations() {
        for is_black in [false, true] {
            let mut app = App::new("me".into());
            app.start_game("opponent".into(), is_black);
            app.update_game(mate(false));
            let area = Rect::new(0, 0, 100, 30);
            let mut before = Buffer::empty(area);
            app.render(area, &mut before);
            app.finish_game("Checkmate".into());
            let mut after = Buffer::empty(area);
            app.render(area, &mut after);
            // Full layout: help is 18 columns, side panel is 26. The board never moves.
            for y in 0..30 {
                for x in 18..74 {
                    assert_eq!(before[(x, y)], after[(x, y)]);
                }
            }
            let rendered = text(&after);
            assert!(rendered.contains("Checkmate"));
            assert!(rendered.contains("Qh4#"));
            assert!(rendered.contains("ENTER: lobby"));
            assert!(!rendered.contains("/resign"));
            let fen = app.game().unwrap().fen();
            assert_eq!(
                app.handle_input(InputEvent::Key(KeyCode::Char('e'), KeyModifiers::NONE)),
                AppAction::None
            );
            assert_eq!(app.game().unwrap().fen(), fen);
            assert_eq!(app.view(), AppView::GameOver);
            assert_eq!(
                app.handle_input(InputEvent::Key(KeyCode::Enter, KeyModifiers::NONE)),
                AppAction::ReturnToLobby
            );
            assert_eq!(app.view(), AppView::Lobby);
            assert!(app.game().is_none());
            app.start_game("next".into(), false);
            assert!(app.game_over_reason.is_none());
            assert_eq!(app.game().unwrap().move_count(), 0);
        }
    }

    #[test]
    fn compact_terminal_shows_final_position_result_and_navigation() {
        let mut app = App::new("me".into());
        app.start_game("opponent".into(), false);
        app.update_game(mate(false));
        app.finish_game("Checkmate".into());
        let mut buffer = Buffer::empty(Rect::new(0, 0, 44, 24));
        app.render(buffer.area, &mut buffer);
        let rendered = text(&buffer);
        assert!(rendered.contains("YOU LOSE - Checkmate"));
        assert!(rendered.contains("ENTER: lobby | Q: quit"));
        assert!(rendered.contains('♚'));
        assert!(rendered.contains('♔'));
    }

    #[test]
    fn unicode_input_does_not_panic_and_promoted_positions_render() {
        let mut app = App::new("me".into());
        app.start_solo_game();
        app.handle_input(InputEvent::Key(KeyCode::Char('💥'), KeyModifiers::NONE));
        app.update_game(Game::from_fen("Q6k/8/8/8/8/8/8/3Q3K b - - 0 1").unwrap());
        let mut buffer = Buffer::empty(Rect::new(0, 0, 100, 30));
        app.render(buffer.area, &mut buffer);
    }
}
