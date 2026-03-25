use crate::app::{App, LoadState};
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect, t: &Theme) {
    match &app.comments_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading comments…")
                .style(t.text_dim_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style())
                        .style(t.background_style()),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("  Error loading comments: {e}"))
                .style(t.diff_removed_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style())
                        .style(t.background_style()),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.pr_comments.is_empty() {
        let p = Paragraph::new("  No review comments on this PR.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(" Comments ", Style::default().fg(t.text_dim))),
        );
        f.render_widget(p, area);
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    for comment in &app.pr_comments {
        // ── comment header ────────────────────────────────────────────────────
        let location = match (&comment.path, comment.line) {
            (Some(p), Some(l)) => format!("  {}:{}", p, l),
            (Some(p), None) => format!("  {}", p),
            _ => String::new(),
        };

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                comment.author.clone(),
                Style::default()
                    .fg(t.pr_author)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(location, Style::default().fg(t.text_accent)),
            Span::styled(
                format!("  {}", comment.created_at),
                Style::default().fg(t.text_dim),
            ),
        ]));

        // ── comment body ──────────────────────────────────────────────────────
        for body_line in comment.body.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", body_line),
                Style::default().fg(t.text),
            )));
        }

        // ── separator ─────────────────────────────────────────────────────────
        lines.push(Line::from(Span::styled(
            " ".to_string() + &"─".repeat(48),
            t.separator_style(),
        )));
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(
                    format!(" Comments ({}) ", app.pr_comments.len()),
                    Style::default().fg(t.text_dim),
                )),
        )
        .scroll((app.comments_scroll, 0));

    f.render_widget(p, area);
}
