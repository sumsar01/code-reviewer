use crate::app::{App, LoadState};
use crate::github::{DiffFile, DiffLineKind};
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const SPLIT_THRESHOLD: u16 = 160;

pub fn render(f: &mut Frame, app: &mut App, area: Rect, t: &Theme) {
    match &app.diff_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading diff…")
                .style(t.text_dim_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style()),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("  Error loading diff: {e}"))
                .style(t.diff_removed_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style()),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.diff_files.is_empty() {
        let p = Paragraph::new("  No diff available.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style()),
        );
        f.render_widget(p, area);
        return;
    }

    let total_files = app.diff_files.len();
    let cur_file_idx = app.diff_file_cursor.min(total_files.saturating_sub(1));
    let file = &app.diff_files[cur_file_idx];
    let title = format!(" {} ({}/{}) ", file.filename, cur_file_idx + 1, total_files);

    let wide = area.width >= SPLIT_THRESHOLD;
    if wide {
        render_side_by_side(f, file, app.diff_scroll, area, &title, t);
    } else {
        render_unified(f, file, app.diff_scroll, area, &title, t);
    }
}

fn render_side_by_side(
    f: &mut Frame,
    file: &DiffFile,
    scroll: u16,
    area: Rect,
    title: &str,
    t: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (left_lines, right_lines) = build_side_by_side_lines(file, t);

    let left_p = Paragraph::new(left_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .title(Span::styled(format!("{title} before"), t.text_dim_style())),
        )
        .scroll((scroll, 0));
    let right_p = Paragraph::new(right_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .title(Span::styled(" after ", t.text_dim_style())),
        )
        .scroll((scroll, 0));

    f.render_widget(left_p, chunks[0]);
    f.render_widget(right_p, chunks[1]);
}

fn render_unified(f: &mut Frame, file: &DiffFile, scroll: u16, area: Rect, title: &str, t: &Theme) {
    let lines = build_unified_lines(file, t);
    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .title(Span::styled(title.to_string(), t.text_accent_style())),
        )
        .scroll((scroll, 0));
    f.render_widget(p, area);
}

fn build_side_by_side_lines(
    file: &DiffFile,
    t: &Theme,
) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    let mut left: Vec<Line<'static>> = Vec::new();
    let mut right: Vec<Line<'static>> = Vec::new();

    for hunk in &file.hunks {
        left.push(Line::from(Span::styled(
            hunk.header.clone(),
            t.diff_hunk_style(),
        )));
        right.push(Line::from(Span::styled(
            hunk.header.clone(),
            t.diff_hunk_style(),
        )));

        for dl in &hunk.lines {
            match dl.kind {
                DiffLineKind::Added => {
                    left.push(Line::from(""));
                    right.push(Line::from(Span::styled(
                        format!(
                            "{:>4} + {}",
                            dl.right_no.map(|n| n.to_string()).unwrap_or_default(),
                            dl.content
                        ),
                        t.diff_added_style(),
                    )));
                }
                DiffLineKind::Removed => {
                    left.push(Line::from(Span::styled(
                        format!(
                            "{:>4} - {}",
                            dl.left_no.map(|n| n.to_string()).unwrap_or_default(),
                            dl.content
                        ),
                        t.diff_removed_style(),
                    )));
                    right.push(Line::from(""));
                }
                DiffLineKind::Context => {
                    let ln = format!(
                        "{:>4}   {}",
                        dl.left_no.map(|n| n.to_string()).unwrap_or_default(),
                        dl.content
                    );
                    left.push(Line::from(Span::styled(ln, t.diff_context_style())));
                    let rn = format!(
                        "{:>4}   {}",
                        dl.right_no.map(|n| n.to_string()).unwrap_or_default(),
                        dl.content
                    );
                    right.push(Line::from(Span::styled(rn, t.diff_context_style())));
                }
            }
        }
    }

    (left, right)
}

fn build_unified_lines(file: &DiffFile, t: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for hunk in &file.hunks {
        lines.push(Line::from(Span::styled(
            hunk.header.clone(),
            t.diff_hunk_style(),
        )));

        for dl in &hunk.lines {
            let (prefix, style) = match dl.kind {
                DiffLineKind::Added => ("+", t.diff_added_style()),
                DiffLineKind::Removed => ("-", t.diff_removed_style()),
                DiffLineKind::Context => (" ", t.diff_context_style()),
            };
            let text = format!("{prefix}{}", dl.content);
            lines.push(Line::from(Span::styled(text, style)));
        }
    }

    lines
}
