use crate::app::App;
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Render the read-only comment-peek popup over the diff view.
/// Displays comments attached to the cursor diff line.
pub fn render(f: &mut Frame, app: &App, t: &Theme) {
    let Some(comments) = &app.comment_peek else {
        return;
    };

    // ── Size the popup ────────────────────────────────────────────────────────
    let area = f.area();
    let popup_width = (area.width * 3 / 4)
        .max(40)
        .min(area.width.saturating_sub(4));

    // Count required lines: header + body lines + separator per comment.
    let content_lines: usize = comments
        .iter()
        .map(|c| 1 + c.body.lines().count() + 1)
        .sum::<usize>()
        .saturating_sub(1); // no trailing separator on last comment
    let content_lines = content_lines.max(1);
    let popup_height = (content_lines as u16 + 2/* borders */).min(area.height.saturating_sub(4));

    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // ── Build content lines ────────────────────────────────────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();
    let last_idx = comments.len().saturating_sub(1);

    for (idx, comment) in comments.iter().enumerate() {
        // Header: author + location + timestamp
        let location = match (&comment.path, comment.line) {
            (Some(p), Some(l)) => format!("  {}:{}", p, l),
            (Some(p), None) => format!("  {}", p),
            _ => String::new(),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {}", comment.author),
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

        // Body
        for body_line in comment.body.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", body_line),
                Style::default().fg(t.text),
            )));
        }

        // Separator between comments (not after the last one)
        if idx < last_idx {
            lines.push(Line::from(Span::styled(
                "\u{2500}".repeat(popup_width.saturating_sub(2) as usize),
                Style::default().fg(t.separator),
            )));
        }
    }

    // ── Render ────────────────────────────────────────────────────────────────
    let title = format!(
        " {} comment{} ",
        comments.len(),
        if comments.len() == 1 { "" } else { "s" }
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_style())
        .style(t.background_style())
        .title(Span::styled(title, t.text_accent_style()))
        .title_bottom(Span::styled(" any key to close ", t.text_dim_style()));

    f.render_widget(Clear, popup_area);
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let p = Paragraph::new(lines).style(t.background_style());
    f.render_widget(p, inner);
}
