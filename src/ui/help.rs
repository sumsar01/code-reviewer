use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

const HELP_TEXT: &[(&str, &str)] = &[
    ("PR List", ""),
    ("j / k", "Navigate up/down"),
    ("Enter", "Open PR detail"),
    ("a", "Toggle mine / all PRs"),
    ("r", "Refresh PR list"),
    ("o", "Open PR in browser"),
    ("q", "Quit"),
    ("", ""),
    ("PR Detail", ""),
    ("Tab", "Switch Diff ↔ Comments ↔ Difftastic"),
    ("j / k", "Scroll down / up"),
    ("h / l", "Scroll left / right"),
    ("<N>j/k/h/l", "Repeat motion N times (e.g. 20j)"),
    ("Ctrl-d / Ctrl-u", "Half page down / up"),
    ("gg / G", "Jump to top / bottom"),
    ("0 / $", "Scroll to line start / end"),
    ("n / N", "Next / previous changed file"),
    ("c", "Checkout PR branch"),
    ("o", "Open PR in browser"),
    ("v", "Toggle file reviewed"),
    ("", ""),
    ("Reviews", ""),
    ("A", "Approve PR (opens input overlay)"),
    ("R", "Request changes (opens input overlay)"),
    ("C", "Leave a review comment (opens input overlay)"),
    (
        "i",
        "Inline comment on cursor diff line (opens input overlay)",
    ),
    ("Enter", "Peek at existing comments on cursor diff line"),
    ("", ""),
    ("Review overlay", ""),
    ("Enter", "Insert newline"),
    ("Ctrl+Enter / Ctrl+S", "Submit review"),
    ("Esc", "Cancel / close overlay"),
    ("", ""),
    ("Both", ""),
    ("Esc / q", "Back to list"),
    ("T", "Open theme picker"),
    ("?", "Toggle this help"),
];

pub fn render(f: &mut Frame, t: &Theme) {
    let area = centered_rect(60, 70, f.area());

    f.render_widget(Clear, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (key, desc) in HELP_TEXT {
        if desc.is_empty() {
            if key.is_empty() {
                lines.push(Line::from(""));
            } else {
                // Section header
                lines.push(Line::from(Span::styled(
                    format!("  {key}"),
                    Style::default()
                        .fg(t.section_header)
                        .add_modifier(Modifier::BOLD),
                )));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", key), t.key_badge_style()),
                Span::styled(format!("  {desc}"), t.key_desc_style()),
            ]));
        }
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(
                    " Help — press ? to close ",
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .alignment(Alignment::Left);

    f.render_widget(p, area);
}

/// Compute a centered `Rect` of given percentage width/height within `r`.
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
