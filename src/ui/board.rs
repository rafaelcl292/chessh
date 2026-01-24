use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use shakmaty::{Chess, File, Position, Rank, Square};

const LIGHT_SQUARE: Color = Color::Rgb(240, 217, 181);
const DARK_SQUARE: Color = Color::Rgb(181, 136, 99);

const WHITE_PIECE: Color = Color::Rgb(255, 255, 255);
const BLACK_PIECE: Color = Color::Rgb(0, 0, 0);

pub struct BoardWidget<'a> {
    position: &'a Chess,
    selected_square: Option<Square>,
    last_move: Option<(Square, Square)>,
    flipped: bool,
}

impl<'a> BoardWidget<'a> {
    pub fn new(position: &'a Chess) -> Self {
        Self {
            position,
            selected_square: None,
            last_move: None,
            flipped: false,
        }
    }

    pub fn selected(mut self, square: Option<Square>) -> Self {
        self.selected_square = square;
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

        let board_width = 8 * square_width;
        let board_height = 8 * square_height;

        if area.width < board_width || area.height < board_height {
            return;
        }

        let start_x = area.x + (area.width.saturating_sub(board_width)) / 2;
        let start_y = area.y + (area.height.saturating_sub(board_height)) / 2;

        for rank_idx in 0..8u8 {
            for file_idx in 0..8u8 {
                let display_rank = if self.flipped { rank_idx } else { 7 - rank_idx };
                let display_file = if self.flipped { 7 - file_idx } else { file_idx };

                let file = File::new(display_file as u32);
                let rank = Rank::new(display_rank as u32);
                let square = Square::from_coords(file, rank);

                let is_light = (display_file + display_rank) % 2 == 1;
                let mut bg_color = if is_light { LIGHT_SQUARE } else { DARK_SQUARE };

                if let Some((from, to)) = self.last_move {
                    if square == from || square == to {
                        bg_color = Color::Rgb(205, 210, 106);
                    }
                }

                if self.selected_square == Some(square) {
                    bg_color = Color::Rgb(130, 151, 105);
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
