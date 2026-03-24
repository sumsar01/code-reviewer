use crate::app::App;
use crate::ui::theme::{Theme, ALL_THEMES};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, t: &Theme) {
    let area = picker_rect(f.area());
    f.render_widget(Clear, area);

    let n = ALL_THEMES.len();
    // inner area height: n rows + 1 blank + 1 status
    let inner_height = (n + 2) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(inner_height), Constraint::Length(1)])
        .split(area);

    // ── Theme list ────────────────────────────────────────────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from("")); // top padding

    for (i, (id, display)) in ALL_THEMES.iter().enumerate() {
        let is_cursor = i == app.theme_picker_cursor;
        let is_active = *id == app.config.ui.theme.to_lowercase().as_str();

        let cursor_glyph = if is_cursor { "▸ " } else { "  " };
        let check = if is_active { " ✓" } else { "  " };

        let line = if is_cursor {
            Line::from(vec![Span::styled(
                format!(" {cursor_glyph}{display}{check} "),
                Style::default()
                    .fg(t.tab_active)
                    .add_modifier(Modifier::BOLD),
            )])
        } else {
            Line::from(vec![Span::styled(
                format!(" {cursor_glyph}{display}{check} "),
                t.text_dim_style(),
            )])
        };

        lines.push(line);
    }

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_style())
        .title(Span::styled(
            " Theme ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ));

    let p = Paragraph::new(lines).block(list_block);
    f.render_widget(p, chunks[0]);

    // ── Status bar ────────────────────────────────────────────────────────────
    let hints: &[(&str, &str)] = &[("j/k", "navigate"), ("Enter", "apply"), ("Esc", "cancel")];

    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", t.text_dim_style()));
        }
        spans.push(Span::styled(format!(" {key} "), t.key_badge_style()));
        spans.push(Span::styled(format!(" {desc}"), t.key_desc_style()));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), chunks[1]);
}

/// Compute a centered rect for the picker: 36 cols wide, auto-height.
fn picker_rect(r: Rect) -> Rect {
    let height = (ALL_THEMES.len() + 4) as u16; // entries + border + padding + statusbar
    let width = 36u16;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(r.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(r.width.saturating_sub(width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vert[1])[1]
}
