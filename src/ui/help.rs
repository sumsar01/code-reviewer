use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

const HELP_TEXT: &[(&str, &str)] = &[
    // key, description
    ("PR List", ""),
    ("j / k", "Navigate up/down"),
    ("Enter", "Open PR detail"),
    ("a", "Toggle mine / all PRs"),
    ("r", "Refresh PR list"),
    ("o", "Open PR in browser"),
    ("q", "Quit"),
    ("", ""),
    ("PR Detail", ""),
    ("Tab", "Switch Diff ↔ Comments"),
    ("j / k", "Scroll"),
    ("n / N", "Next / previous changed file"),
    ("c", "Checkout PR branch"),
    ("o", "Open PR in browser"),
    ("Esc / q", "Back to list"),
    ("", ""),
    ("Both", ""),
    ("?", "Toggle this help"),
];

pub fn render(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());

    // Clear the area underneath the popup
    f.render_widget(Clear, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (key, desc) in HELP_TEXT {
        if desc.is_empty() {
            // Section header or blank line
            if key.is_empty() {
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("── {} ", key),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<12}", key), Style::default().fg(Color::Yellow)),
                Span::raw(desc.to_string()),
            ]));
        }
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help — press ? to close "),
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
