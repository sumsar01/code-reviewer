use crate::app::{App, DetailFocus, LoadState};
use crate::github::{DiffFile, DiffLineKind, ReviewComment};
use crate::syntax::SyntaxHighlighter;
use crate::ui::{file_tree, theme::Theme};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const SPLIT_THRESHOLD: u16 = 160;
/// Width of the file-tree sidebar in columns.
const TREE_WIDTH: u16 = 36;

pub fn render(f: &mut Frame, app: &mut App, area: Rect, t: &Theme, hl: &SyntaxHighlighter) {
    match &app.diff_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading diff…")
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
            let p = Paragraph::new(format!("  Error loading diff: {e}"))
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

    if app.diff_files.is_empty() {
        let p = Paragraph::new("  No diff available.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style()),
        );
        f.render_widget(p, area);
        return;
    }

    // Split the area when the file tree is visible.
    let diff_area = if app.show_file_tree {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(TREE_WIDTH), Constraint::Min(0)])
            .split(area);

        let paths: Vec<String> = app.diff_files.iter().map(|f| f.filename.clone()).collect();
        let rows = file_tree::build_rows(&paths);
        let focused = app.detail_focus == DetailFocus::FileTree;
        let reviewed = app.reviewed_diff_indices();
        file_tree::render(
            f,
            &rows,
            app.file_tree_cursor,
            app.diff_file_cursor,
            focused,
            chunks[0],
            t,
            &reviewed,
        );

        chunks[1]
    } else {
        area
    };

    let total_files = app.diff_files.len();
    let cur_file_idx = app.diff_file_cursor.min(total_files.saturating_sub(1));
    let file = &app.diff_files[cur_file_idx];
    let base_title = format!(" {} ({}/{}) ", file.filename, cur_file_idx + 1, total_files);

    let is_reviewed = app.reviewed_diff_indices().contains(&cur_file_idx);
    let title: Line<'static> = if is_reviewed {
        Line::from(vec![
            Span::styled(base_title, t.text_accent_style()),
            Span::styled("[reviewed] ", Style::default().fg(Color::Green)),
        ])
    } else {
        Line::from(Span::styled(base_title, t.text_accent_style()))
    };

    let line_cursor = app.diff_line_cursor;
    let wide = diff_area.width >= SPLIT_THRESHOLD;

    // Record the viewport height so Ctrl-d/u can compute half-page.
    // Subtract 2 for the top/bottom borders of the paragraph block.
    app.last_diff_area_height = diff_area.height.saturating_sub(2);

    if wide {
        render_side_by_side(
            f,
            file,
            app.diff_scroll,
            app.diff_hscroll,
            diff_area,
            title,
            t,
            hl,
            line_cursor,
            &app.pr_comments,
        );
    } else {
        render_unified(
            f,
            file,
            app.diff_scroll,
            app.diff_hscroll,
            diff_area,
            title,
            t,
            hl,
            line_cursor,
            &app.pr_comments,
        );
    }
}

// ── Highlight helpers ─────────────────────────────────────────────────────────

/// Reconstruct the "new" and "old" source texts for `file` so that
/// tree-sitter can parse each side as a coherent document.
///
/// * `new_source` — added + context lines (the post-patch file)
/// * `old_source` — removed + context lines (the pre-patch file)
///
/// Also returns per-hunk-line index maps:
/// * `new_idx[hunk][line]` = index into `new_source`'s line list, or `None`
///   if this diff line is a Removed line (not present on the new side).
/// * `old_idx[hunk][line]` = index into `old_source`'s line list, or `None`
///   if this diff line is an Added line (not present on the old side).
fn build_sources_for_file(
    file: &DiffFile,
) -> (
    String,
    String,
    Vec<Vec<Option<usize>>>,
    Vec<Vec<Option<usize>>>,
) {
    let mut new_parts: Vec<&str> = Vec::new();
    let mut old_parts: Vec<&str> = Vec::new();
    let mut new_idx: Vec<Vec<Option<usize>>> = Vec::new();
    let mut old_idx: Vec<Vec<Option<usize>>> = Vec::new();

    for hunk in &file.hunks {
        let mut hunk_new: Vec<Option<usize>> = Vec::new();
        let mut hunk_old: Vec<Option<usize>> = Vec::new();
        for dl in &hunk.lines {
            match dl.kind {
                DiffLineKind::Added => {
                    hunk_new.push(Some(new_parts.len()));
                    hunk_old.push(None);
                    new_parts.push(&dl.content);
                }
                DiffLineKind::Removed => {
                    hunk_new.push(None);
                    hunk_old.push(Some(old_parts.len()));
                    old_parts.push(&dl.content);
                }
                DiffLineKind::Context => {
                    hunk_new.push(Some(new_parts.len()));
                    hunk_old.push(Some(old_parts.len()));
                    new_parts.push(&dl.content);
                    old_parts.push(&dl.content);
                }
            }
        }
        new_idx.push(hunk_new);
        old_idx.push(hunk_old);
    }

    (new_parts.join("\n"), old_parts.join("\n"), new_idx, old_idx)
}

/// Given a source string pre-highlighted for `filename`, and the byte offset
/// of `content` within that source, produce a `Vec<Span<'static>>` whose
/// foreground colours reflect the syntax token under each character.
///
/// `diff_bg` is the background applied uniformly to the whole line.
/// `fallback_fg` is the colour used for any character that isn't covered by a
/// syntax token (e.g. the prefix `+`/`-`/` ` and line numbers).
fn spans_for_content(
    content: &str,
    line_hl_spans: &[crate::syntax::HlSpan],
    diff_bg: Option<ratatui::style::Color>,
    fallback_fg: ratatui::style::Color,
    t: &Theme,
) -> Vec<Span<'static>> {
    let bytes = content.as_bytes();
    let len = bytes.len();

    if line_hl_spans.is_empty() || len == 0 {
        let style = match diff_bg {
            Some(bg) => Style::default().fg(fallback_fg).bg(bg),
            None => Style::default().fg(fallback_fg),
        };
        return vec![Span::styled(content.to_owned(), style)];
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut pos = 0usize;

    for hl_span in line_hl_spans {
        let start = hl_span.start.min(len);
        let end = hl_span.end.min(len);
        if end <= start {
            continue;
        }

        // Gap before this span — use fallback color
        if pos < start {
            let text = content[pos..start].to_owned();
            let style = match diff_bg {
                Some(bg) => Style::default().fg(fallback_fg).bg(bg),
                None => Style::default().fg(fallback_fg),
            };
            spans.push(Span::styled(text, style));
        }

        let fg = match hl_span.hl_index {
            Some(idx) => t.syntax_color(idx),
            None => fallback_fg,
        };
        let style = match diff_bg {
            Some(bg) => Style::default().fg(fg).bg(bg),
            None => Style::default().fg(fg),
        };
        let text = content[start..end].to_owned();
        spans.push(Span::styled(text, style));
        pos = end;
    }

    // Trailing text after the last span
    if pos < len {
        let text = content[pos..].to_owned();
        let style = match diff_bg {
            Some(bg) => Style::default().fg(fallback_fg).bg(bg),
            None => Style::default().fg(fallback_fg),
        };
        spans.push(Span::styled(text, style));
    }

    spans
}

// ── Unified view ──────────────────────────────────────────────────────────────

fn render_unified(
    f: &mut Frame,
    file: &DiffFile,
    scroll: u16,
    hscroll: u16,
    area: Rect,
    title: Line<'static>,
    t: &Theme,
    hl: &SyntaxHighlighter,
    line_cursor: usize,
    comments: &[ReviewComment],
) {
    let lines = build_unified_lines(file, t, hl, line_cursor, comments);
    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(title),
        )
        .scroll((scroll, hscroll));
    f.render_widget(p, area);
}

fn build_unified_lines(
    file: &DiffFile,
    t: &Theme,
    hl: &SyntaxHighlighter,
    line_cursor: usize,
    comments: &[ReviewComment],
) -> Vec<Line<'static>> {
    let (new_source, old_source, new_idx, old_idx) = build_sources_for_file(file);
    let new_hl = hl.highlight(&file.filename, &new_source);
    let old_hl = hl.highlight(&file.filename, &old_source);

    // Build a set of (line_number, side) pairs that have existing comments so
    // we can annotate them with a marker.
    let commented: std::collections::HashSet<(u64, &str)> = comments
        .iter()
        .filter(|c| c.path.as_deref() == Some(&file.filename))
        .filter_map(|c| c.line.map(|ln| (ln, "RIGHT")))
        .collect();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut row: usize = 0; // rendered-row counter matching diff_line_cursor

    for (hunk_i, hunk) in file.hunks.iter().enumerate() {
        // Hunk-header row
        let is_cursor = row == line_cursor;
        let hunk_style = if is_cursor {
            t.diff_hunk_style().bg(t.selection_bg)
        } else {
            t.diff_hunk_style()
        };
        let hunk_header = if is_cursor {
            format!("\u{25b6} {}", hunk.header.trim_start())
        } else {
            format!("  {}", hunk.header.trim_start())
        };
        lines.push(Line::from(Span::styled(hunk_header, hunk_style)));
        row += 1;

        for (line_i, dl) in hunk.lines.iter().enumerate() {
            let is_cursor = row == line_cursor;

            let (prefix, diff_fg, diff_bg) = match dl.kind {
                DiffLineKind::Added => ("+", t.diff_added_fg, Some(t.diff_added_bg)),
                DiffLineKind::Removed => ("-", t.diff_removed_fg, Some(t.diff_removed_bg)),
                DiffLineKind::Context => (" ", t.diff_context, None),
            };

            // When the cursor is on this row, use selection_bg as the background
            // for a subtle highlight; keep the diff colour for added/removed lines.
            let effective_bg: Option<Color> = if is_cursor {
                Some(t.selection_bg)
            } else {
                diff_bg
            };

            // Gutter: show ▶ on cursor line, space otherwise.
            let gutter = if is_cursor { "\u{25b6}" } else { " " };
            let line_no_str = match dl.kind {
                DiffLineKind::Added => format!(
                    "{}{:>4} ",
                    gutter,
                    dl.right_no.map(|n| n.to_string()).unwrap_or_default()
                ),
                DiffLineKind::Removed => format!(
                    "{}{:>4} ",
                    gutter,
                    dl.left_no.map(|n| n.to_string()).unwrap_or_default()
                ),
                DiffLineKind::Context => format!(
                    "{}{:>4} ",
                    gutter,
                    dl.left_no.map(|n| n.to_string()).unwrap_or_default()
                ),
            };

            // Gutter indicator uses accent colour on cursor line so it's visible.
            let gutter_fg = if is_cursor { t.text_accent } else { diff_fg };
            let prefix_style = match effective_bg {
                Some(bg) => Style::default().fg(gutter_fg).bg(bg),
                None => Style::default().fg(gutter_fg),
            };
            let content_style = match effective_bg {
                Some(bg) => Style::default().fg(diff_fg).bg(bg),
                None => Style::default().fg(diff_fg),
            };

            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::styled(line_no_str, prefix_style));
            spans.push(Span::styled(prefix.to_string(), content_style));

            // Pick the correct highlight data: removed lines use old_hl, others use new_hl.
            let hl_span_opt = match dl.kind {
                DiffLineKind::Removed => {
                    let src_idx = old_idx[hunk_i][line_i];
                    match (&old_hl, src_idx) {
                        (Some(data), Some(idx)) if idx < data.len() => Some(&data[idx] as &[_]),
                        _ => None,
                    }
                }
                _ => {
                    let src_idx = new_idx[hunk_i][line_i];
                    match (&new_hl, src_idx) {
                        (Some(data), Some(idx)) if idx < data.len() => Some(&data[idx] as &[_]),
                        _ => None,
                    }
                }
            };

            match hl_span_opt {
                Some(line_hl) => {
                    spans.extend(spans_for_content(
                        &dl.content,
                        line_hl,
                        effective_bg,
                        diff_fg,
                        t,
                    ));
                }
                None => {
                    let style = match effective_bg {
                        Some(bg) => Style::default().fg(diff_fg).bg(bg),
                        None => Style::default().fg(diff_fg),
                    };
                    spans.push(Span::styled(dl.content.clone(), style));
                }
            }

            // Append a comment-annotation marker if this line has existing comments.
            let comment_line_no = match dl.kind {
                DiffLineKind::Removed => dl.left_no.map(|n| (n as u64, "LEFT")),
                _ => dl.right_no.map(|n| (n as u64, "RIGHT")),
            };
            let has_comment = comment_line_no
                .map(|(ln, side)| commented.contains(&(ln, side)))
                .unwrap_or(false);
            if has_comment {
                let marker_bg = effective_bg.unwrap_or(t.background);
                spans.push(Span::styled(
                    " \u{25cf}".to_string(), // ● bullet
                    Style::default().fg(t.text_accent).bg(marker_bg),
                ));
            }

            lines.push(Line::from(spans));
            row += 1;
        }
    }

    lines
}

// ── Side-by-side view ─────────────────────────────────────────────────────────

fn render_side_by_side(
    f: &mut Frame,
    file: &DiffFile,
    scroll: u16,
    hscroll: u16,
    area: Rect,
    title: Line<'static>,
    t: &Theme,
    hl: &SyntaxHighlighter,
    line_cursor: usize,
    comments: &[ReviewComment],
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (left_lines, right_lines) = build_side_by_side_lines(file, t, hl, line_cursor, comments);

    let left_p = Paragraph::new(left_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(title)
                .title_bottom(Span::styled(" before ", t.text_dim_style())),
        )
        .scroll((scroll, hscroll));
    let right_p = Paragraph::new(right_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(" after ", t.text_dim_style())),
        )
        .scroll((scroll, hscroll));

    f.render_widget(left_p, chunks[0]);
    f.render_widget(right_p, chunks[1]);
}

fn build_side_by_side_lines(
    file: &DiffFile,
    t: &Theme,
    hl: &SyntaxHighlighter,
    line_cursor: usize,
    comments: &[ReviewComment],
) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    let (new_source, old_source, new_idx, old_idx) = build_sources_for_file(file);
    let new_hl = hl.highlight(&file.filename, &new_source);
    let old_hl = hl.highlight(&file.filename, &old_source);

    // Build set of commented (line_no, side) pairs for this file.
    let commented: std::collections::HashSet<(u64, &str)> = comments
        .iter()
        .filter(|c| c.path.as_deref() == Some(&file.filename))
        .filter_map(|c| c.line.map(|ln| (ln, "RIGHT")))
        .collect();

    let mut left: Vec<Line<'static>> = Vec::new();
    let mut right: Vec<Line<'static>> = Vec::new();
    let mut row: usize = 0;

    for (hunk_i, hunk) in file.hunks.iter().enumerate() {
        let is_cursor = row == line_cursor;
        let hunk_style = if is_cursor {
            t.diff_hunk_style().bg(t.selection_bg)
        } else {
            t.diff_hunk_style()
        };
        let hunk_header = if is_cursor {
            format!("\u{25b6} {}", hunk.header.trim_start())
        } else {
            format!("  {}", hunk.header.trim_start())
        };
        left.push(Line::from(Span::styled(hunk_header.clone(), hunk_style)));
        right.push(Line::from(Span::styled(hunk_header, hunk_style)));
        row += 1;

        for (line_i, dl) in hunk.lines.iter().enumerate() {
            let is_cursor = row == line_cursor;
            let gutter = if is_cursor { "\u{25b6}" } else { " " };

            match dl.kind {
                DiffLineKind::Added => {
                    let effective_bg = if is_cursor {
                        t.selection_bg
                    } else {
                        t.diff_added_bg
                    };
                    left.push(Line::from(""));

                    let no_str = format!(
                        "{}{:>4} + ",
                        gutter,
                        dl.right_no.map(|n| n.to_string()).unwrap_or_default()
                    );
                    let gutter_fg = if is_cursor {
                        t.text_accent
                    } else {
                        t.diff_added_fg
                    };
                    let prefix_style = Style::default().fg(gutter_fg).bg(effective_bg);
                    let mut spans = vec![Span::styled(no_str, prefix_style)];
                    let line_hl: &[crate::syntax::HlSpan] = match (&new_hl, new_idx[hunk_i][line_i])
                    {
                        (Some(data), Some(idx)) if idx < data.len() => &data[idx],
                        _ => &[],
                    };
                    spans.extend(spans_for_content(
                        &dl.content,
                        line_hl,
                        Some(effective_bg),
                        t.diff_added_fg,
                        t,
                    ));
                    // Comment marker
                    if dl
                        .right_no
                        .map(|n| commented.contains(&(n as u64, "RIGHT")))
                        .unwrap_or(false)
                    {
                        spans.push(Span::styled(
                            " \u{25cf}".to_string(),
                            Style::default().fg(t.text_accent).bg(effective_bg),
                        ));
                    }
                    right.push(Line::from(spans));
                }
                DiffLineKind::Removed => {
                    let effective_bg = if is_cursor {
                        t.selection_bg
                    } else {
                        t.diff_removed_bg
                    };
                    let no_str = format!(
                        "{}{:>4} - ",
                        gutter,
                        dl.left_no.map(|n| n.to_string()).unwrap_or_default()
                    );
                    let gutter_fg = if is_cursor {
                        t.text_accent
                    } else {
                        t.diff_removed_fg
                    };
                    let prefix_style = Style::default().fg(gutter_fg).bg(effective_bg);
                    let mut spans = vec![Span::styled(no_str, prefix_style)];
                    let line_hl: &[crate::syntax::HlSpan] = match (&old_hl, old_idx[hunk_i][line_i])
                    {
                        (Some(data), Some(idx)) if idx < data.len() => &data[idx],
                        _ => &[],
                    };
                    spans.extend(spans_for_content(
                        &dl.content,
                        line_hl,
                        Some(effective_bg),
                        t.diff_removed_fg,
                        t,
                    ));
                    // Comment marker (LEFT side)
                    if dl
                        .left_no
                        .map(|n| commented.contains(&(n as u64, "LEFT")))
                        .unwrap_or(false)
                    {
                        spans.push(Span::styled(
                            " \u{25cf}".to_string(),
                            Style::default().fg(t.text_accent).bg(effective_bg),
                        ));
                    }
                    left.push(Line::from(spans));
                    right.push(Line::from(""));
                }
                DiffLineKind::Context => {
                    let effective_bg_opt: Option<Color> = if is_cursor {
                        Some(t.selection_bg)
                    } else {
                        None
                    };
                    let lno_str = format!(
                        "{}{:>4}   ",
                        gutter,
                        dl.left_no.map(|n| n.to_string()).unwrap_or_default()
                    );
                    let rno_str = format!(
                        "{}{:>4}   ",
                        gutter,
                        dl.right_no.map(|n| n.to_string()).unwrap_or_default()
                    );
                    let gutter_fg = if is_cursor {
                        t.text_accent
                    } else {
                        t.diff_context
                    };
                    let no_style = match effective_bg_opt {
                        Some(bg) => Style::default().fg(gutter_fg).bg(bg),
                        None => Style::default().fg(t.diff_context),
                    };
                    // Context lines are the same on both sides; use new_hl.
                    let line_hl: &[crate::syntax::HlSpan] = match (&new_hl, new_idx[hunk_i][line_i])
                    {
                        (Some(data), Some(idx)) if idx < data.len() => &data[idx],
                        _ => &[],
                    };

                    let mut lspans = vec![Span::styled(lno_str, no_style)];
                    lspans.extend(spans_for_content(
                        &dl.content,
                        line_hl,
                        effective_bg_opt,
                        t.diff_context,
                        t,
                    ));
                    left.push(Line::from(lspans));

                    let mut rspans = vec![Span::styled(rno_str, no_style)];
                    rspans.extend(spans_for_content(
                        &dl.content,
                        line_hl,
                        effective_bg_opt,
                        t.diff_context,
                        t,
                    ));
                    // Comment marker for context lines (right side)
                    if dl
                        .right_no
                        .map(|n| commented.contains(&(n as u64, "RIGHT")))
                        .unwrap_or(false)
                    {
                        let marker_bg = effective_bg_opt.unwrap_or(t.background);
                        rspans.push(Span::styled(
                            " \u{25cf}".to_string(),
                            Style::default().fg(t.text_accent).bg(marker_bg),
                        ));
                    }
                    right.push(Line::from(rspans));
                }
            }
            row += 1;
        }
    }

    (left, right)
}
