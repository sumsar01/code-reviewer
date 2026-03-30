//! Update-available prompt overlay.
//!
//! Shown when the background update check finds a newer GitHub Release.
//! The user can press [y] to install the update or [n]/Esc to dismiss.

use crate::app::App;
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, t: &Theme) {
    let area = centered_rect(50, 30, f.area());
    f.render_widget(Clear, area);

    let current = env!("CARGO_PKG_VERSION");
    let new_tag = match &app.update_available {
        Some(tag) => tag.clone(),
        None => return,
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        format!("  A new version {new_tag} is available!"),
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )));

    lines.push(Line::from(Span::styled(
        format!("  Current version: v{current}"),
        t.key_desc_style(),
    )));

    lines.push(Line::from(""));

    if app.update_in_progress {
        lines.push(Line::from(Span::styled(
            "  Updating… this may take a minute.",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::ITALIC),
        )));
        lines.push(Line::from(Span::styled(
            "  Please wait.",
            t.key_desc_style(),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  [y] ", t.key_badge_style()),
            Span::styled("Update now    ", t.key_desc_style()),
            Span::styled("[n] ", t.key_badge_style()),
            Span::styled("Skip", t.key_desc_style()),
        ]));
    }

    lines.push(Line::from(""));

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(
                    " Update Available ",
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .alignment(Alignment::Left);

    f.render_widget(p, area);
}

/// Compute a centered `Rect` of the given percentage width/height within `r`.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
