use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

const BG: Color = Color::Rgb(24, 27, 32);
const TEXT: Color = Color::Rgb(229, 224, 210);
const MUTED: Color = Color::Rgb(145, 157, 157);
const ACCENT: Color = Color::Rgb(151, 203, 166);
const PANEL: Color = Color::Rgb(34, 43, 43);

pub struct LobbyView<'a> {
    pub online_count: usize,
    pub queue_size: usize,
    pub username: &'a str,
    pub searching: bool,
    pub selected: usize,
    pub help: bool,
    pub command: &'a str,
    pub status: Option<&'a str>,
}

impl LobbyView<'_> {
    pub fn menu_inner(area: Rect) -> Rect {
        let width = area.width.min(96);
        let height = area.height.min(29);
        let content = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(content);
        let body = if width >= 76 && height >= 23 {
            Layout::horizontal([Constraint::Percentage(43), Constraint::Percentage(57)])
                .split(rows[1])[1]
        } else {
            rows[1]
        };
        Block::default().borders(Borders::ALL).inner(body)
    }

    pub fn option_at(area: Rect, x: u16, y: u16) -> Option<usize> {
        let inner = Self::menu_inner(area);
        if !inner.contains((x, y).into()) {
            return None;
        }
        let row = y.checked_sub(inner.y + 2)?;
        let step = if inner.height < 17 { 2 } else { 3 };
        let index = row / step;
        (index < 4 && row % step < step - 1).then_some(index as usize)
    }
}

impl Widget for LobbyView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(BG).fg(TEXT))
            .render(area, buf);
        let width = area.width.min(96);
        let height = area.height.min(29);
        let content = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(content);
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    " CheSSH",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  /  Lobby", Style::default().fg(MUTED)),
            ]),
            Line::from(format!(
                " {} online  ·  {} in queue",
                self.online_count, self.queue_size
            )),
        ])
        .render(rows[0], buf);

        let wide = content.width >= 76 && content.height >= 23;
        let body = if wide {
            let columns =
                Layout::horizontal([Constraint::Percentage(43), Constraint::Percentage(57)])
                    .split(rows[1]);
            Paragraph::new(vec![
                Line::from(""),
                Line::from("       ▄▄▄▄"),
                Line::from("     ▄██████▄"),
                Line::from("   ▄██▀ ▀████"),
                Line::from("  ███▄▄██████"),
                Line::from("   ▀▀▀ ██████"),
                Line::from("      ███████"),
                Line::from("     ████████"),
                Line::from("   ▄██████████▄"),
                Line::from("   ▀▀▀▀▀▀▀▀▀▀▀▀"),
                Line::from(""),
                Line::from(Span::styled(
                    "Your next move starts here.",
                    Style::default().fg(TEXT),
                )),
                Line::from(Span::styled(
                    "Chess, straight from your terminal.",
                    Style::default().fg(MUTED),
                )),
            ])
            .style(Style::default().fg(ACCENT))
            .alignment(Alignment::Center)
            .render(columns[0], buf);
            columns[1]
        } else {
            rows[1]
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(67, 85, 78)))
            .title(if self.searching {
                " Matchmaking "
            } else if self.help {
                " How to play "
            } else {
                " Make your move "
            });
        let inner = Self::menu_inner(area);
        block.render(body, buf);
        let compact = inner.height < 17;
        let mut lines = vec![];
        if self.searching {
            lines.extend([
                Line::from(""),
                Line::from(Span::styled(
                    "  Searching for opponent...",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("  You are in the online queue."),
                Line::from("  Your game starts when a player joins."),
                Line::from(""),
                Line::from("  Open a second terminal to play a friend."),
                Line::from(""),
                Line::from(Span::styled(
                    "  [ Enter / Esc / h ]  Cancel search",
                    Style::default().fg(ACCENT),
                )),
            ]);
        } else if self.help {
            lines.extend([
                Line::from(Span::styled(
                    "  A quick guide",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("  Online: get matched with another player."),
                Line::from("  Practice: control both sides. No AI."),
                Line::from(""),
                Line::from("  Type a move, then press Enter:"),
                Line::from("  e4 · Nf3 · O-O  or  e2e4"),
                Line::from("  During a game: /draw · /resign · /lobby"),
                Line::from("  After a game: review the final board."),
                Line::from(""),
                Line::from(Span::styled(
                    "  [ Enter / Esc / h ]  Back to menu",
                    Style::default().fg(ACCENT),
                )),
            ]);
        } else {
            lines.push(Line::from(Span::styled(
                format!("  Welcome, {}", self.username),
                Style::default().fg(MUTED),
            )));
            lines.push(Line::from(""));
            for (index, (label, description)) in [
                ("Play online", "Find another player and start a game"),
                ("Practice board", "Explore moves and play both sides"),
                ("How to play", "A quick guide to your first game"),
                ("Disconnect", "See you next time"),
            ]
            .iter()
            .enumerate()
            {
                let selected = self.selected == index;
                let style = if selected {
                    Style::default()
                        .fg(ACCENT)
                        .bg(PANEL)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT)
                };
                lines.push(Line::from(Span::styled(
                    format!(
                        " {} {}  {}",
                        if selected { "▸" } else { " " },
                        index + 1,
                        label
                    ),
                    style,
                )));
                if !compact {
                    lines.push(Line::from(Span::styled(
                        format!("      {description}"),
                        Style::default().fg(MUTED),
                    )));
                }
                lines.push(Line::from(""));
            }
            if let Some(status) = self.status {
                lines.push(Line::from(Span::styled(
                    status,
                    Style::default().fg(ACCENT),
                )));
            }
        }
        if self.help || self.searching {
            lines.pop();
            let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .render(rows[0], buf);
            Paragraph::new(if self.searching {
                "  Cancel search  [h / Esc]"
            } else {
                "  Back to menu  [h / Esc]"
            })
            .style(Style::default().fg(ACCENT).bg(PANEL))
            .render(rows[1], buf);
        } else {
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .render(inner, buf);
        }
        let footer = if !self.command.is_empty() {
            format!("Command: {}_  ·  Esc cancel", self.command)
        } else if self.help || self.searching {
            "Enter / Esc / h  back     Ctrl+C  disconnect".into()
        } else if content.width < 60 {
            "j/k ↑↓ select   l/Enter open   h back".into()
        } else {
            "j/k ↑↓ Tab  select   l/Enter →  open   h/←  back   1–4  shortcuts".into()
        };
        Paragraph::new(footer)
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center)
            .render(rows[2], buf);
    }
}
