use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use shakmaty::{Chess, File, Position, Rank, Square};

const LIGHT_SQUARE: Color = Color::Rgb(240, 217, 181);
const DARK_SQUARE: Color = Color::Rgb(181, 136, 99);
const SELECTED_SQUARE: Color = Color::Rgb(130, 151, 105);
const LAST_MOVE_SQUARE: Color = Color::Rgb(205, 210, 106);
const LEGAL_MOVE_LIGHT: Color = Color::Rgb(170, 162, 131);
const LEGAL_MOVE_DARK: Color = Color::Rgb(141, 111, 81);
const CHECK_SQUARE: Color = Color::Rgb(220, 80, 80);

const WHITE_PIECE: Color = Color::Rgb(255, 255, 255);
const BLACK_PIECE: Color = Color::Rgb(0, 0, 0);

pub struct BoardWidget<'a> {
    position: &'a Chess,
    selected_square: Option<Square>,
    last_move: Option<(Square, Square)>,
    legal_moves: Vec<Square>,
    flipped: bool,
}

impl<'a> BoardWidget<'a> {
    pub fn new(position: &'a Chess) -> Self {
        Self {
            position,
            selected_square: None,
            last_move: None,
            legal_moves: Vec::new(),
            flipped: false,
        }
    }

    pub fn selected(mut self, square: Option<Square>) -> Self {
        self.selected_square = square;
        if let Some(sq) = square {
            self.legal_moves = self
                .position
                .legal_moves()
                .iter()
                .filter(|m| m.from() == Some(sq))
                .map(|m| m.to())
                .collect();
        } else {
            self.legal_moves.clear();
        }
        self
    }

    pub fn last_move(mut self, from: Square, to: Square) -> Self {
        self.last_move = Some((from, to));
        self
    }

    pub fn flipped(mut self, flipped: bool) -> Self {
        self.flipped = flipped;
        self
    }

    fn is_king_in_check(&self, square: Square) -> bool {
        if !self.position.is_check() {
            return false;
        }
        if let Some(piece) = self.position.board().piece_at(square) {
            return piece.role == shakmaty::Role::King && piece.color == self.position.turn();
        }
        false
    }

    fn piece_char(role: shakmaty::Role, color: shakmaty::Color) -> char {
        match (color, role) {
            (shakmaty::Color::White, shakmaty::Role::King) => '♔',
            (shakmaty::Color::White, shakmaty::Role::Queen) => '♕',
            (shakmaty::Color::White, shakmaty::Role::Rook) => '♖',
            (shakmaty::Color::White, shakmaty::Role::Bishop) => '♗',
            (shakmaty::Color::White, shakmaty::Role::Knight) => '♘',
            (shakmaty::Color::White, shakmaty::Role::Pawn) => '♙',
            (shakmaty::Color::Black, shakmaty::Role::King) => '♚',
            (shakmaty::Color::Black, shakmaty::Role::Queen) => '♛',
            (shakmaty::Color::Black, shakmaty::Role::Rook) => '♜',
            (shakmaty::Color::Black, shakmaty::Role::Bishop) => '♝',
            (shakmaty::Color::Black, shakmaty::Role::Knight) => '♞',
            (shakmaty::Color::Black, shakmaty::Role::Pawn) => '♟',
        }
    }
}

impl Widget for BoardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let square_width = 4u16;
        let square_height = 2u16;

        let board_width = 8 * square_width + 2;
        let board_height = 8 * square_height + 1;

        if area.width < board_width || area.height < board_height {
            return;
        }

        let start_x = area.x + (area.width.saturating_sub(board_width)) / 2 + 2;
        let start_y = area.y + (area.height.saturating_sub(board_height)) / 2;

        for rank_idx in 0..8u8 {
            let display_rank = if self.flipped { rank_idx } else { 7 - rank_idx };
            let rank_label = (b'1' + display_rank) as char;
            let label_y = start_y + rank_idx as u16 * square_height + square_height / 2;
            if start_x >= 2 && label_y < area.y + area.height {
                buf[(start_x - 2, label_y)].set_char(rank_label);
            }
        }

        for file_idx in 0..8u8 {
            let display_file = if self.flipped { 7 - file_idx } else { file_idx };
            let file_label = (b'a' + display_file) as char;
            let label_x = start_x + file_idx as u16 * square_width + square_width / 2;
            let label_y = start_y + 8 * square_height;
            if label_x < area.x + area.width && label_y < area.y + area.height {
                buf[(label_x, label_y)].set_char(file_label);
            }
        }

        for rank_idx in 0..8u8 {
            for file_idx in 0..8u8 {
                let display_rank = if self.flipped { rank_idx } else { 7 - rank_idx };
                let display_file = if self.flipped { 7 - file_idx } else { file_idx };

                let file = File::new(display_file as u32);
                let rank = Rank::new(display_rank as u32);
                let square = Square::from_coords(file, rank);

                let is_light = (display_file + display_rank) % 2 == 1;
                let mut bg_color = if is_light { LIGHT_SQUARE } else { DARK_SQUARE };

                if self.is_king_in_check(square) {
                    bg_color = CHECK_SQUARE;
                } else if let Some((from, to)) = self.last_move {
                    if square == from || square == to {
                        bg_color = LAST_MOVE_SQUARE;
                    }
                }

                if self.legal_moves.contains(&square) {
                    bg_color = if is_light {
                        LEGAL_MOVE_LIGHT
                    } else {
                        LEGAL_MOVE_DARK
                    };
                }

                if self.selected_square == Some(square) {
                    bg_color = SELECTED_SQUARE;
                }

                let x = start_x + file_idx as u16 * square_width;
                let y = start_y + rank_idx as u16 * square_height;

                for dy in 0..square_height {
                    for dx in 0..square_width {
                        if x + dx < area.x + area.width && y + dy < area.y + area.height {
                            buf[(x + dx, y + dy)].set_bg(bg_color).set_char(' ');
                        }
                    }
                }

                if let Some(piece) = self.position.board().piece_at(square) {
                    let piece_char = Self::piece_char(piece.role, piece.color);
                    let piece_color = match piece.color {
                        shakmaty::Color::White => WHITE_PIECE,
                        shakmaty::Color::Black => BLACK_PIECE,
                    };

                    let px = x + square_width / 2;
                    let py = y + square_height / 2;

                    if px < area.x + area.width && py < area.y + area.height {
                        buf[(px, py)]
                            .set_char(piece_char)
                            .set_fg(piece_color)
                            .set_bg(bg_color);
                    }
                }
            }
        }
    }
}
