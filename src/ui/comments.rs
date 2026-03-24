use crate::app::{App, LoadState};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    match &app.comments_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("Loading comments…")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("Error loading comments: {e}"))
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.pr_comments.is_empty() {
        let p = Paragraph::new("No review comments on this PR.")
            .block(Block::default().borders(Borders::ALL).title(" Comments "));
        f.render_widget(p, area);
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    for comment in &app.pr_comments {
        // ── comment header ─────────────────────────────────────────────
        let location = match (&comment.path, comment.line) {
            (Some(p), Some(l)) => format!("  {}:{}", p, l),
            (Some(p), None) => format!("  {}", p),
            _ => String::new(),
        };

        lines.push(Line::from(vec![
            Span::styled(
                comment.author.clone(),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(location, Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("  {}", comment.created_at),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // ── comment body ───────────────────────────────────────────────
        for body_line in comment.body.lines() {
            lines.push(Line::from(Span::raw(format!("  {}", body_line))));
        }

        // ── separator ──────────────────────────────────────────────────
        lines.push(Line::from(Span::styled(
            "─".repeat(60),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Comments ({}) ", app.pr_comments.len())),
        )
        .scroll((app.comments_scroll, 0));

    f.render_widget(p, area);
}
