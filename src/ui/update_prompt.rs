//! Update-available prompt overlay.
//!
//! Shown when the background update check finds a newer GitHub Release.
//! The user can press [y] to confirm the update (exits the TUI and runs
//! `cargo install` in a clean terminal) or [n]/Esc to dismiss.

use crate::app::App;
use crate::ui::{theme::Theme, utils::centered_rect};
use ratatui::{
    layout::Alignment,
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

    lines.push(Line::from(vec![
        Span::styled("  [y] ", t.key_badge_style()),
        Span::styled("Update now    ", t.key_desc_style()),
        Span::styled("[n] ", t.key_badge_style()),
        Span::styled("Skip", t.key_desc_style()),
    ]));

    lines.push(Line::from(Span::styled(
        "  (exits prr, then runs cargo install)",
        t.text_dim_style(),
    )));

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
