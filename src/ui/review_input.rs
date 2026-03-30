use crate::app::{App, ReviewAction};
use crate::ui::{theme::Theme, utils::render_hint_bar};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Modal width as a percentage of the terminal width.
const MODAL_WIDTH_PCT: u16 = 65;
/// Modal height as a percentage of the terminal height.
const MODAL_HEIGHT_PCT: u16 = 55;
/// Minimum modal width in terminal columns.
const MODAL_MIN_WIDTH: u16 = 50;
/// Minimum modal height in terminal rows.
const MODAL_MIN_HEIGHT: u16 = 12;

/// Render the review-input overlay modal.
pub fn render(f: &mut Frame, app: &App, t: &Theme) {
    let Some(overlay) = &app.review_overlay else {
        return;
    };

    let area = modal_rect(f.area());
    f.render_widget(Clear, area);

    // Split: [title bar] [text area] [status/error bar] [hint bar]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title padding (inside block)
            Constraint::Min(3),    // text area
            Constraint::Length(1), // error / status line
            Constraint::Length(1), // hint bar
        ])
        .margin(1)
        .split(area);

    // ── Outer block ──────────────────────────────────────────────────────────
    let accent = action_color(&overlay.action, t);
    let title = match &overlay.action {
        ReviewAction::InlineComment { path, line, side } => {
            let side_label = if side == "LEFT" { "old" } else { "new" };
            format!(" Inline Comment  {}:{} ({}) ", path, line, side_label)
        }
        _ => format!(" {} ", overlay.action.title()),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(t.background_style())
        .title(Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(block, area);

    // ── Text area ─────────────────────────────────────────────────────────────
    let text_area = chunks[1];
    let view_height = text_area.height as usize;

    // Scroll so the cursor is always visible.
    let cursor_row = overlay.cursor_row;
    let scroll_top = if cursor_row >= view_height {
        cursor_row - view_height + 1
    } else {
        0
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (row_idx, line_str) in overlay.lines.iter().enumerate() {
        if row_idx < scroll_top {
            continue;
        }
        if lines.len() >= view_height {
            break;
        }

        if row_idx == cursor_row {
            // Render the cursor line with a blinking block cursor.
            let col = overlay.cursor_col;
            let before: String = line_str[..col].to_string();
            let cursor_ch: String = line_str[col..]
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string());
            let after: String = if col < line_str.len() {
                let ch_end = col + line_str[col..].chars().next().map_or(0, |c| c.len_utf8());
                line_str[ch_end..].to_string()
            } else {
                String::new()
            };

            lines.push(Line::from(vec![
                Span::styled(before, t.text_style()),
                Span::styled(
                    cursor_ch,
                    Style::default()
                        .fg(t.background)
                        .bg(accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(after, t.text_style()),
            ]));
        } else {
            lines.push(Line::from(Span::styled(line_str.clone(), t.text_style())));
        }
    }

    // Fill remaining rows with empty lines so the box looks uniform.
    while lines.len() < view_height {
        lines.push(Line::from(""));
    }

    let text_para = Paragraph::new(lines).style(t.background_style());
    f.render_widget(text_para, text_area);

    // ── Error / hint line ─────────────────────────────────────────────────────
    if let Some(err) = &overlay.error {
        let err_line = Paragraph::new(Line::from(Span::styled(
            format!(" ⚠ {err}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )))
        .style(t.background_style());
        f.render_widget(err_line, chunks[2]);
    }

    // ── Key hint bar ──────────────────────────────────────────────────────────
    let submit_label = match &overlay.action {
        ReviewAction::Approve => "Ctrl+Enter / Ctrl+S  Approve",
        ReviewAction::RequestChanges => "Ctrl+Enter / Ctrl+S  Request Changes",
        ReviewAction::InlineComment { .. } => "Ctrl+Enter / Ctrl+S  Post Comment",
        _ => "Ctrl+Enter / Ctrl+S  Submit Comment",
    };

    let hints: &[(&str, &str)] = &[(submit_label, ""), ("Esc", "cancel"), ("Enter", "newline")];

    render_hint_bar(f, hints, chunks[3], t);
}

/// Pick an accent color based on the review action.
fn action_color(action: &ReviewAction, t: &Theme) -> Color {
    match action {
        ReviewAction::Approve => Color::Green,
        ReviewAction::RequestChanges => Color::Red,
        ReviewAction::Comment | ReviewAction::InlineComment { .. } => t.text_accent,
    }
}

/// Compute a centered modal rect (MODAL_WIDTH_PCT% width, MODAL_HEIGHT_PCT% height).
fn modal_rect(r: Rect) -> Rect {
    let width = (r.width * MODAL_WIDTH_PCT / 100)
        .max(MODAL_MIN_WIDTH)
        .min(r.width);
    let height = (r.height * MODAL_HEIGHT_PCT / 100)
        .max(MODAL_MIN_HEIGHT)
        .min(r.height);

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
