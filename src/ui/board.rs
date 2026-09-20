use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use shakmaty::{Chess, File, Position, Rank, Square};

use super::sprites::{get_sprite, render_sprite_half_blocks};

const LIGHT_SQUARE: Color = Color::Rgb(240, 217, 181);
const DARK_SQUARE: Color = Color::Rgb(181, 136, 99);
const SELECTED_SQUARE: Color = Color::Rgb(130, 151, 105);
const LAST_MOVE_SQUARE: Color = Color::Rgb(205, 210, 106);
const LEGAL_MOVE_LIGHT: Color = Color::Rgb(170, 162, 131);
const LEGAL_MOVE_DARK: Color = Color::Rgb(141, 111, 81);
const CHECK_SQUARE: Color = Color::Rgb(220, 80, 80);
const CANDIDATE_ORIGIN_LIGHT: Color = Color::Rgb(180, 180, 130);
const CANDIDATE_ORIGIN_DARK: Color = Color::Rgb(150, 130, 90);

const WHITE_PIECE: Color = Color::Rgb(255, 255, 255);
const BLACK_PIECE: Color = Color::Rgb(0, 0, 0);

pub struct BoardWidget<'a> {
    position: &'a Chess,
    selected_square: Option<Square>,
    last_move: Option<(Square, Square)>,
    legal_moves: Vec<Square>,
    candidate_origins: Vec<Square>,
    flipped: bool,
}

impl<'a> BoardWidget<'a> {
    pub fn new(position: &'a Chess) -> Self {
        Self {
            position,
            selected_square: None,
            last_move: None,
            legal_moves: Vec::new(),
            candidate_origins: Vec::new(),
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

    pub fn highlights(mut self, origins: Vec<Square>, destinations: Vec<Square>) -> Self {
        self.candidate_origins = origins;
        self.legal_moves = destinations;
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

    fn geometry(area: Rect) -> Option<(u16, u16, u16, u16, bool)> {
        let sprite_square_width = 10u16;
        let sprite_square_height = 5u16;

        let sprite_board_width = 8 * sprite_square_width + 2;
        let sprite_board_height = 8 * sprite_square_height + 1;

        let use_sprites = area.width >= sprite_board_width && area.height >= sprite_board_height;

        let (square_width, square_height, board_width, board_height) = if use_sprites {
            (
                sprite_square_width,
                sprite_square_height,
                sprite_board_width,
                sprite_board_height,
            )
        } else {
            let sw = 4u16;
            let sh = 2u16;
            (sw, sh, 8 * sw + 2, 8 * sh + 1)
        };

        if area.width < board_width || area.height < board_height {
            return None;
        }

        let start_x = area.x + (area.width.saturating_sub(board_width)) / 2 + 2;
        let start_y = area.y + (area.height.saturating_sub(board_height)) / 2;

        Some((start_x, start_y, square_width, square_height, use_sprites))
    }

    pub fn square_at(area: Rect, x: u16, y: u16, flipped: bool) -> Option<Square> {
        let (left, top, width, height, _) = Self::geometry(area)?;
        let col = x.checked_sub(left)? / width;
        let row = y.checked_sub(top)? / height;
        if col >= 8 || row >= 8 {
            return None;
        }
        Some(Square::from_coords(
            File::new(if flipped { 7 - col } else { col } as u32),
            Rank::new(if flipped { row } else { 7 - row } as u32),
        ))
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
        let Some((start_x, start_y, square_width, square_height, use_sprites)) =
            Self::geometry(area)
        else {
            return;
        };

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

                if self.candidate_origins.contains(&square) {
                    bg_color = if is_light {
                        CANDIDATE_ORIGIN_LIGHT
                    } else {
                        CANDIDATE_ORIGIN_DARK
                    };
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
                    if use_sprites {
                        if let Some(sprite) = get_sprite(piece.color, piece.role) {
                            let half_blocks = render_sprite_half_blocks(sprite, bg_color);
                            for (row_idx, row) in half_blocks.iter().enumerate() {
                                for (col_idx, hb) in row.iter().enumerate() {
                                    let px = x + col_idx as u16;
                                    let py = y + row_idx as u16;
                                    if px < area.x + area.width && py < area.y + area.height {
                                        buf[(px, py)].set_char(hb.char).set_fg(hb.fg).set_bg(hb.bg);
                                    }
                                }
                            }
                        }
                    } else {
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
}
