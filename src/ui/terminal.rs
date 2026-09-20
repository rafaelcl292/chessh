use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{KeyCode, KeyModifiers},
    style::{
        Attribute, Color as CrosstermColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{Clear, ClearType},
};
use ratatui::backend::Backend;
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};
use tokio::sync::mpsc;

pub struct SshWriter {
    tx: mpsc::Sender<Vec<u8>>,
    buffer: Vec<u8>,
}

impl SshWriter {
    pub fn new(tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            tx,
            buffer: Vec::with_capacity(4096),
        }
    }

    pub fn do_flush(&mut self) {
        if !self.buffer.is_empty() {
            let data = std::mem::take(&mut self.buffer);
            let _ = self.tx.try_send(data);
        }
    }
}

impl Write for SshWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.do_flush();
        Ok(())
    }
}

pub struct SshBackend {
    writer: SshWriter,
    size: Size,
}

impl SshBackend {
    pub fn new(tx: mpsc::Sender<Vec<u8>>, width: u16, height: u16) -> Self {
        // Colors encode board squares and piece sides on the remote terminal.
        // The server's NO_COLOR setting must not strip them from SSH output.
        crossterm::style::force_color_output(true);
        Self {
            writer: SshWriter::new(tx),
            size: Size { width, height },
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.size = Size { width, height };
    }

    fn write_command<C: crossterm::Command>(&mut self, cmd: C) -> io::Result<()> {
        let mut buf = String::new();
        if cmd.write_ansi(&mut buf).is_err() {
            return Err(io::Error::other("Failed to write ANSI"));
        }
        self.writer.write_all(buf.as_bytes())
    }

    fn color_to_crossterm(color: Color) -> CrosstermColor {
        match color {
            Color::Reset => CrosstermColor::Reset,
            Color::Black => CrosstermColor::Black,
            Color::Red => CrosstermColor::DarkRed,
            Color::Green => CrosstermColor::DarkGreen,
            Color::Yellow => CrosstermColor::DarkYellow,
            Color::Blue => CrosstermColor::DarkBlue,
            Color::Magenta => CrosstermColor::DarkMagenta,
            Color::Cyan => CrosstermColor::DarkCyan,
            Color::Gray => CrosstermColor::Grey,
            Color::DarkGray => CrosstermColor::DarkGrey,
            Color::LightRed => CrosstermColor::Red,
            Color::LightGreen => CrosstermColor::Green,
            Color::LightYellow => CrosstermColor::Yellow,
            Color::LightBlue => CrosstermColor::Blue,
            Color::LightMagenta => CrosstermColor::Magenta,
            Color::LightCyan => CrosstermColor::Cyan,
            Color::White => CrosstermColor::White,
            Color::Rgb(r, g, b) => CrosstermColor::Rgb { r, g, b },
            Color::Indexed(i) => CrosstermColor::AnsiValue(i),
        }
    }
}

impl Backend for SshBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut last_pos: Option<(u16, u16)> = None;
        let mut last_fg = Color::Reset;
        let mut last_bg = Color::Reset;
        let mut last_modifier = Modifier::empty();

        for (x, y, cell) in content {
            let should_move = match last_pos {
                Some((lx, ly)) => !(lx + 1 == x && ly == y),
                None => true,
            };

            if should_move {
                self.write_command(cursor::MoveTo(x, y))?;
            }

            if cell.fg != last_fg {
                self.write_command(SetForegroundColor(Self::color_to_crossterm(cell.fg)))?;
                last_fg = cell.fg;
            }

            if cell.bg != last_bg {
                self.write_command(SetBackgroundColor(Self::color_to_crossterm(cell.bg)))?;
                last_bg = cell.bg;
            }

            if cell.modifier != last_modifier {
                if cell.modifier.contains(Modifier::BOLD) && !last_modifier.contains(Modifier::BOLD)
                {
                    self.write_command(SetAttribute(Attribute::Bold))?;
                }
                if cell.modifier.contains(Modifier::DIM) && !last_modifier.contains(Modifier::DIM) {
                    self.write_command(SetAttribute(Attribute::Dim))?;
                }
                if cell.modifier.contains(Modifier::UNDERLINED)
                    && !last_modifier.contains(Modifier::UNDERLINED)
                {
                    self.write_command(SetAttribute(Attribute::Underlined))?;
                }
                if cell.modifier.contains(Modifier::REVERSED)
                    && !last_modifier.contains(Modifier::REVERSED)
                {
                    self.write_command(SetAttribute(Attribute::Reverse))?;
                }
                if !cell.modifier.contains(Modifier::BOLD) && last_modifier.contains(Modifier::BOLD)
                {
                    self.write_command(SetAttribute(Attribute::NormalIntensity))?;
                }
                if !cell.modifier.contains(Modifier::UNDERLINED)
                    && last_modifier.contains(Modifier::UNDERLINED)
                {
                    self.write_command(SetAttribute(Attribute::NoUnderline))?;
                }
                if !cell.modifier.contains(Modifier::REVERSED)
                    && last_modifier.contains(Modifier::REVERSED)
                {
                    self.write_command(SetAttribute(Attribute::NoReverse))?;
                }
                last_modifier = cell.modifier;
            }

            write!(self.writer, "{}", cell.symbol())?;
            last_pos = Some((x, y));
        }

        self.write_command(SetForegroundColor(CrosstermColor::Reset))?;
        self.write_command(SetBackgroundColor(CrosstermColor::Reset))?;
        self.write_command(SetAttribute(Attribute::Reset))?;

        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.write_command(cursor::Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.write_command(cursor::Show)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(Position { x: 0, y: 0 })
    }

    fn set_cursor_position<P: Into<ratatui::layout::Position>>(
        &mut self,
        position: P,
    ) -> io::Result<()> {
        let pos = position.into();
        self.write_command(cursor::MoveTo(pos.x, pos.y))
    }

    fn clear(&mut self) -> io::Result<()> {
        self.write_command(Clear(ClearType::All))
    }

    fn clear_region(&mut self, clear_type: ratatui::backend::ClearType) -> io::Result<()> {
        use ratatui::backend::ClearType as RatatuiClearType;
        let crossterm_clear_type = match clear_type {
            RatatuiClearType::All => ClearType::All,
            RatatuiClearType::AfterCursor => ClearType::FromCursorDown,
            RatatuiClearType::BeforeCursor => ClearType::FromCursorUp,
            RatatuiClearType::CurrentLine => ClearType::CurrentLine,
            RatatuiClearType::UntilNewLine => ClearType::UntilNewLine,
        };
        self.write_command(Clear(crossterm_clear_type))
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn window_size(&mut self) -> io::Result<ratatui::backend::WindowSize> {
        Ok(ratatui::backend::WindowSize {
            columns_rows: self.size,
            pixels: Size::default(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyCode, KeyModifiers),
    Resize(u16, u16),
    Click(u16, u16),
    Unknown,
}

pub fn parse_input(data: &[u8]) -> InputEvent {
    if data.is_empty() {
        return InputEvent::Unknown;
    }

    if data.len() >= 6 && data[0] == 0xFF && data[1] == 0xFE {
        let width = u16::from_be_bytes([data[2], data[3]]);
        let height = u16::from_be_bytes([data[4], data[5]]);
        return InputEvent::Resize(width, height);
    }

    if data.starts_with(b"\x1b[<") {
        let Some(body) = data.strip_suffix(b"M") else {
            return InputEvent::Unknown;
        };
        let Ok(body) = std::str::from_utf8(&body[3..]) else {
            return InputEvent::Unknown;
        };
        let values: Vec<_> = body.split(';').map(str::parse::<u16>).collect();
        if let [Ok(0), Ok(x), Ok(y)] = values.as_slice() {
            if *x > 0 && *y > 0 {
                return InputEvent::Click(x - 1, y - 1);
            }
        }
        return InputEvent::Unknown;
    }

    match data {
        [0x03] => InputEvent::Key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        [0x04] => InputEvent::Key(KeyCode::Char('d'), KeyModifiers::CONTROL),
        [0x0D] | [0x0A] => InputEvent::Key(KeyCode::Enter, KeyModifiers::NONE),
        [0x1B] => InputEvent::Key(KeyCode::Esc, KeyModifiers::NONE),
        [0x7F] | [0x08] => InputEvent::Key(KeyCode::Backspace, KeyModifiers::NONE),
        [0x09] => InputEvent::Key(KeyCode::Tab, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x5A] => InputEvent::Key(KeyCode::BackTab, KeyModifiers::SHIFT),
        [0x1B, 0x4F, 0x41] => InputEvent::Key(KeyCode::Up, KeyModifiers::NONE),
        [0x1B, 0x4F, 0x42] => InputEvent::Key(KeyCode::Down, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x41] => InputEvent::Key(KeyCode::Up, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x42] => InputEvent::Key(KeyCode::Down, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x43] => InputEvent::Key(KeyCode::Right, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x44] => InputEvent::Key(KeyCode::Left, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x48] => InputEvent::Key(KeyCode::Home, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x46] => InputEvent::Key(KeyCode::End, KeyModifiers::NONE),
        [0x1B, 0x5B, 0x33, 0x7E] => InputEvent::Key(KeyCode::Delete, KeyModifiers::NONE),
        [c] if c.is_ascii() && *c >= 0x20 => {
            InputEvent::Key(KeyCode::Char(*c as char), KeyModifiers::NONE)
        }
        [c] if *c >= 0x01 && *c <= 0x1A => {
            let ch = (b'a' + c - 1) as char;
            InputEvent::Key(KeyCode::Char(ch), KeyModifiers::CONTROL)
        }
        _ => {
            if let Ok(s) = std::str::from_utf8(data) {
                if let Some(c) = s.chars().next() {
                    return InputEvent::Key(KeyCode::Char(c), KeyModifiers::NONE);
                }
            }
            InputEvent::Unknown
        }
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;

    #[test]
    fn ssh_output_preserves_foreground_and_background_colors() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut backend = SshBackend::new(tx, 80, 24);
        let mut cell = Cell::default();
        cell.set_symbol("▀")
            .set_fg(Color::Rgb(151, 203, 166))
            .set_bg(Color::Rgb(181, 136, 99));
        backend.draw([(0, 0, &cell)].into_iter()).unwrap();
        backend.flush().unwrap();
        let output = String::from_utf8(rx.try_recv().unwrap()).unwrap();
        assert!(output.contains("38;2;151;203;166"));
        assert!(output.contains("48;2;181;136;99"));
    }
}

/// SSH data packets are not event boundaries: retain fragmented mouse reports.
#[derive(Default)]
pub struct InputDecoder {
    pending: Vec<u8>,
}
impl InputDecoder {
    pub fn flush_escape(&mut self) -> Option<InputEvent> {
        if self.pending == b"\x1b" {
            self.pending.clear();
            Some(InputEvent::Key(KeyCode::Esc, KeyModifiers::NONE))
        } else {
            None
        }
    }

    pub fn feed(&mut self, data: &[u8]) -> Vec<InputEvent> {
        if data.starts_with(&[0xff, 0xfe]) {
            return vec![parse_input(data)];
        }
        self.pending.extend_from_slice(data);
        let mut events = Vec::new();
        while !self.pending.is_empty() {
            if self.pending == b"\x1b" {
                break;
            }
            let length = if self.pending.starts_with(b"\x1b[") || self.pending.starts_with(b"\x1bO")
            {
                match self
                    .pending
                    .iter()
                    .enumerate()
                    .skip(2)
                    .find(|(_, c)| (0x40..=0x7e).contains(*c))
                {
                    Some((i, _)) => i + 1,
                    None if self.pending.len() < 64 => break,
                    None => {
                        self.pending.clear();
                        break;
                    }
                }
            } else if self.pending[0] >= 0x80 {
                let expected = match self.pending[0] {
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => 1,
                };
                if self.pending.len() < expected {
                    break;
                }
                expected
            } else {
                1
            };
            events.push(parse_input(&self.pending[..length]));
            self.pending.drain(..length);
        }
        events
    }
}

#[cfg(test)]
mod mouse_tests {
    use super::*;
    #[test]
    fn mouse_reports_survive_every_packet_boundary_and_ignore_releases() {
        let report = b"\x1b[<0;120;45M";
        for split in 1..report.len() {
            let mut decoder = InputDecoder::default();
            assert!(decoder.feed(&report[..split]).is_empty());
            assert_eq!(
                decoder.feed(&report[split..]),
                vec![InputEvent::Click(119, 44)]
            );
        }
        let mut decoder = InputDecoder::default();
        assert_eq!(
            decoder.feed(b"\x1b[<0;1;1M\x1b[<0;1;1m"),
            vec![InputEvent::Click(0, 0), InputEvent::Unknown]
        );
        assert_eq!(parse_input(b"\x1b[<0;0;1M"), InputEvent::Unknown);
        assert_eq!(parse_input(b"\x1b[<64;1;1M"), InputEvent::Unknown);
        assert!(decoder.feed(b"\x1b").is_empty());
        assert_eq!(
            decoder.flush_escape(),
            Some(InputEvent::Key(KeyCode::Esc, KeyModifiers::NONE))
        );
    }
}
