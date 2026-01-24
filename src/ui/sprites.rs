use image::{Rgb, RgbImage};
use ratatui::style::Color;
use shakmaty::{Color as ChessColor, Role};
use std::sync::OnceLock;

static SPRITES: OnceLock<SpriteCache> = OnceLock::new();

const SPRITE_WIDTH: u32 = 10;
const SPRITE_HEIGHT: u32 = 10;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum SpriteRow {
    Brown = 0,
    Yellow = 1,
    DarkBlue = 2,
    LightBlue = 3,
}

impl SpriteRow {
    pub fn for_color(color: ChessColor) -> Self {
        match color {
            ChessColor::White => SpriteRow::LightBlue,
            ChessColor::Black => SpriteRow::DarkBlue,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SpriteColumn {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl SpriteColumn {
    pub fn from_role(role: Role) -> Self {
        match role {
            Role::Pawn => SpriteColumn::Pawn,
            Role::Knight => SpriteColumn::Knight,
            Role::Bishop => SpriteColumn::Bishop,
            Role::Rook => SpriteColumn::Rook,
            Role::Queen => SpriteColumn::Queen,
            Role::King => SpriteColumn::King,
        }
    }
}

#[derive(Clone)]
pub struct Sprite {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Rgb<u8>>,
}

impl Sprite {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Rgb([0, 0, 0]); width * height],
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Rgb<u8> {
        self.pixels[y * self.width + x]
    }
}

pub struct SpriteCache {
    sprites: [[Sprite; 6]; 4],
}

impl SpriteCache {
    fn load() -> Option<Self> {
        let img_data = include_bytes!("../../assets/pieces.png");
        let img = image::load_from_memory(img_data).ok()?;
        let rgb_img = img.to_rgb8();

        let mut sprites = vec![];
        for row in 0..4 {
            for col in 0..6 {
                let x = col * SPRITE_WIDTH;
                let y = row * SPRITE_HEIGHT;
                let sprite = Self::extract_sprite(&rgb_img, x, y);
                sprites.push(sprite);
            }
        }

        let sprites_array: Vec<[Sprite; 6]> = sprites
            .chunks(6)
            .map(|chunk| {
                [
                    chunk[0].clone(),
                    chunk[1].clone(),
                    chunk[2].clone(),
                    chunk[3].clone(),
                    chunk[4].clone(),
                    chunk[5].clone(),
                ]
            })
            .collect();

        Some(Self {
            sprites: [
                sprites_array[0].clone(),
                sprites_array[1].clone(),
                sprites_array[2].clone(),
                sprites_array[3].clone(),
            ],
        })
    }

    fn extract_sprite(img: &RgbImage, x_offset: u32, y_offset: u32) -> Sprite {
        let mut sprite = Sprite::new(SPRITE_WIDTH as usize, SPRITE_HEIGHT as usize);
        for y in 0..SPRITE_HEIGHT {
            for x in 0..SPRITE_WIDTH {
                let pixel = img.get_pixel(x_offset + x, y_offset + y);
                sprite.pixels[(y * SPRITE_WIDTH + x) as usize] = *pixel;
            }
        }
        sprite
    }

    pub fn get(&self, row: SpriteRow, col: SpriteColumn) -> &Sprite {
        &self.sprites[row as usize][col as usize]
    }
}

pub fn init_sprites() {
    SPRITES.get_or_init(|| SpriteCache::load().expect("Failed to load sprites"));
}

pub fn get_sprite(color: ChessColor, role: Role) -> Option<&'static Sprite> {
    let cache = SPRITES.get()?;
    let row = SpriteRow::for_color(color);
    let col = SpriteColumn::from_role(role);
    Some(cache.get(row, col))
}

pub fn rgb_to_ansi(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

#[derive(Debug, Clone, Copy)]
pub struct HalfBlockPixel {
    pub char: char,
    pub fg: Color,
    pub bg: Color,
}

pub fn render_sprite_half_blocks(sprite: &Sprite, bg_color: Color) -> Vec<Vec<HalfBlockPixel>> {
    let rows = (sprite.height + 1) / 2;
    let mut output = vec![vec![]; rows];

    for row in 0..rows {
        for col in 0..sprite.width {
            let top_y = row * 2;
            let bottom_y = row * 2 + 1;

            let top_pixel = sprite.pixel(col, top_y);
            let bottom_pixel = if bottom_y < sprite.height {
                sprite.pixel(col, bottom_y)
            } else {
                Rgb([0, 0, 0])
            };

            let top_transparent = is_transparent(&top_pixel);
            let bottom_transparent = is_transparent(&bottom_pixel);

            let half_block = match (top_transparent, bottom_transparent) {
                (true, true) => HalfBlockPixel {
                    char: ' ',
                    fg: bg_color,
                    bg: bg_color,
                },
                (false, true) => HalfBlockPixel {
                    char: '▀',
                    fg: rgb_to_ansi(top_pixel[0], top_pixel[1], top_pixel[2]),
                    bg: bg_color,
                },
                (true, false) => HalfBlockPixel {
                    char: '▄',
                    fg: rgb_to_ansi(bottom_pixel[0], bottom_pixel[1], bottom_pixel[2]),
                    bg: bg_color,
                },
                (false, false) => {
                    if pixels_similar(&top_pixel, &bottom_pixel) {
                        HalfBlockPixel {
                            char: '█',
                            fg: rgb_to_ansi(top_pixel[0], top_pixel[1], top_pixel[2]),
                            bg: bg_color,
                        }
                    } else {
                        HalfBlockPixel {
                            char: '▀',
                            fg: rgb_to_ansi(top_pixel[0], top_pixel[1], top_pixel[2]),
                            bg: rgb_to_ansi(bottom_pixel[0], bottom_pixel[1], bottom_pixel[2]),
                        }
                    }
                }
            };

            output[row].push(half_block);
        }
    }

    output
}

fn is_transparent(pixel: &Rgb<u8>) -> bool {
    pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0
}

fn pixels_similar(a: &Rgb<u8>, b: &Rgb<u8>) -> bool {
    let diff = ((a[0] as i32 - b[0] as i32).abs()
        + (a[1] as i32 - b[1] as i32).abs()
        + (a[2] as i32 - b[2] as i32).abs()) as u32;
    diff < 30
}
