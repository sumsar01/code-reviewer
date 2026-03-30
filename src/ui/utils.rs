use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Compute a centered `Rect` of given percentage width/height within `r`.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

/// Return badge text and color for a review decision string.
///
/// Uses the canonical long-form labels (e.g. `"✗ CHANGES REQUESTED"`).
pub fn review_badge(decision: &str) -> (&'static str, Color) {
    match decision {
        "APPROVED" => ("✓ APPROVED", Color::Green),
        "CHANGES_REQUESTED" => ("✗ CHANGES REQUESTED", Color::Red),
        "REVIEW_REQUIRED" => ("? REVIEW REQUIRED", Color::Yellow),
        _ => ("? REVIEW REQUIRED", Color::Yellow),
    }
}

/// Render a key-hint status bar into `area`.
///
/// Each entry in `hints` is a `(key_label, description)` pair.
/// Entries with an empty description render the key label only (useful for
/// composite labels like `"Ctrl+Enter / Ctrl+S  Submit"`).
pub fn render_hint_bar(f: &mut Frame, hints: &[(&str, &str)], area: Rect, t: &Theme) {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", t.text_dim_style()));
        }
        spans.push(Span::styled(format!(" {key} "), t.key_badge_style()));
        if !desc.is_empty() {
            spans.push(Span::styled(format!(" {desc}"), t.key_desc_style()));
        }
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(t.background_style()),
        area,
    );
}
