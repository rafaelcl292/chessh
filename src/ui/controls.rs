use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap},
};

pub const BG: Color = Color::Rgb(24, 27, 32);
pub const TEXT: Color = Color::Rgb(229, 224, 210);
pub const ACCENT: Color = Color::Rgb(151, 203, 166);
pub const MUTED: Color = Color::Rgb(145, 157, 157);
pub const BORDER: Color = Color::Rgb(67, 85, 78);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameControl {
    Resign,
    Draw,
    Lobby,
    Disconnect,
}
impl GameControl {
    pub const ALL: [Self; 4] = [Self::Resign, Self::Draw, Self::Lobby, Self::Disconnect];
    pub fn label(self) -> &'static str {
        match self {
            Self::Resign => "F2 Resign",
            Self::Draw => "F3 Draw",
            Self::Lobby => "F4 Lobby",
            Self::Disconnect => "F5 Quit",
        }
    }
}
pub fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(title)
}
pub fn button_areas(area: Rect) -> [Rect; 4] {
    if area.width >= 78 {
        std::array::from_fn(|i| {
            Rect::new(area.x + 1, area.y + 4 + i as u16 * 4, 16, 3).intersection(area)
        })
    } else {
        let footer = Rect::new(
            area.x,
            area.bottom().saturating_sub(3).max(area.y),
            area.width,
            area.height.min(3),
        );
        let cells = Layout::horizontal([Constraint::Percentage(25); 4]).split(footer);
        [cells[0], cells[1], cells[2], cells[3]]
    }
}
pub fn dialog_area(area: Rect) -> Rect {
    let w = area.width.min(52);
    let h = area.height.min(9);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}
pub fn choices(area: Rect) -> [Rect; 2] {
    let inner = panel("").inner(dialog_area(area));
    let row = Rect::new(
        inner.x,
        inner.bottom().saturating_sub(3).max(inner.y),
        inner.width,
        inner.height.min(3),
    );
    let cols = Layout::horizontal([Constraint::Percentage(50); 2]).split(row);
    [cols[0], cols[1]]
}
pub fn confirmation(area: Rect, buf: &mut Buffer, message: &str, confirm_selected: bool) {
    let rect = dialog_area(area);
    Clear.render(rect, buf);
    let block = panel(" Confirm action ").style(Style::default().bg(BG).fg(TEXT));
    let inner = block.inner(rect);
    block.render(rect, buf);
    Paragraph::new(message).wrap(Wrap { trim: true }).render(
        Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(3),
        ),
        buf,
    );
    for (i, rect) in choices(area).into_iter().enumerate() {
        Paragraph::new(if i == 0 {
            "Cancel [Esc]"
        } else {
            "Confirm [Y]"
        })
        .style(Style::default().fg(if (i == 1) == confirm_selected {
            ACCENT
        } else {
            MUTED
        }))
        .block(panel(if (i == 1) == confirm_selected {
            " Selected "
        } else {
            ""
        }))
        .render(rect, buf);
    }
}
