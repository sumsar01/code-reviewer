use crate::app::{App, DetailFocus, LoadState};
use crate::syntax::{HlSpan, SyntaxHighlighter};
use crate::ui::{file_tree, theme::Theme};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Width of the file-tree sidebar in columns.
const TREE_WIDTH: u16 = 36;

pub fn render(f: &mut Frame, app: &mut App, area: Rect, t: &Theme, hl: &SyntaxHighlighter) {
    match &app.difft_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Running difftastic…")
                .style(t.text_dim_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style())
                        .style(t.background_style())
                        .title(Span::styled(
                            " Difftastic ",
                            Style::default().fg(t.text_dim),
                        )),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("  {e}"))
                .style(t.diff_removed_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style())
                        .style(t.background_style())
                        .title(Span::styled(
                            " Difftastic ",
                            Style::default().fg(t.text_dim),
                        )),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.difft_files.is_empty() {
        let p = Paragraph::new("  No difftastic output available.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(
                    " Difftastic ",
                    Style::default().fg(t.text_dim),
                )),
        );
        f.render_widget(p, area);
        return;
    }

    // Split the area when the file tree is visible.
    let content_area = if app.show_file_tree {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(TREE_WIDTH), Constraint::Min(0)])
            .split(area);

        let paths: Vec<String> = app.difft_files.iter().map(|(n, _)| n.clone()).collect();
        let rows = file_tree::build_rows(&paths);
        let focused = app.detail_focus == DetailFocus::FileTree;
        let reviewed = app.reviewed_difft_indices();
        file_tree::render(
            f,
            &rows,
            app.file_tree_cursor,
            app.difft_file_cursor,
            focused,
            chunks[0],
            t,
            &reviewed,
        );

        chunks[1]
    } else {
        area
    };

    let total_files = app.difft_files.len();
    let cur_file_idx = app.difft_file_cursor.min(total_files.saturating_sub(1));
    let (filename, raw_ansi) = &app.difft_files[cur_file_idx];
    let title = format!(" {} ({}/{}) ", filename, cur_file_idx + 1, total_files);

    let lines = parse_ansi_to_lines_with_syntax(raw_ansi, filename, hl, t);

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .style(t.background_style())
                .title(Span::styled(title, t.text_accent_style())),
        )
        .scroll((app.difft_scroll, 0));
    f.render_widget(p, content_area);
}

/// Parse ANSI SGR escape sequences from difftastic output into ratatui `Line`s.
///
/// Handles:
///   - `\x1b[0m`       reset
///   - `\x1b[1m`       bold
///   - `\x1b[3m`       italic
///   - `\x1b[4m`       underline
///   - `\x1b[30–37m`   standard foreground colors
///   - `\x1b[90–97m`   bright foreground colors
///   - `\x1b[38;5;Nm`  256-color foreground
///   - `\x1b[38;2;R;G;Bm` true-color foreground
///   - Multiple codes in one sequence (`\x1b[1;32m`)
pub fn parse_ansi_to_lines(input: &str) -> Vec<ratatui::text::Line<'static>> {
    let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();

    for raw_line in input.lines() {
        let spans = parse_ansi_spans(raw_line);
        lines.push(ratatui::text::Line::from(spans));
    }

    lines
}

fn parse_ansi_spans(input: &str) -> Vec<ratatui::text::Span<'static>> {
    let mut spans: Vec<ratatui::text::Span<'static>> = Vec::new();
    let mut current_style = ratatui::style::Style::default();
    let mut text_start = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            if i > text_start {
                let text = input[text_start..i].to_string();
                spans.push(ratatui::text::Span::styled(text, current_style));
            }

            let seq_start = i + 2;
            let mut seq_end = seq_start;
            while seq_end < len && !bytes[seq_end].is_ascii_alphabetic() {
                seq_end += 1;
            }

            if seq_end < len && bytes[seq_end] == b'm' {
                let codes_str = &input[seq_start..seq_end];
                current_style = apply_sgr_codes(current_style, codes_str);
            }

            i = seq_end + 1;
            text_start = i;
        } else {
            i += 1;
        }
    }

    if text_start < len {
        let text = input[text_start..].to_string();
        spans.push(ratatui::text::Span::styled(text, current_style));
    }

    if spans.is_empty() {
        spans.push(ratatui::text::Span::raw(String::new()));
    }

    spans
}

fn apply_sgr_codes(mut style: ratatui::style::Style, codes_str: &str) -> ratatui::style::Style {
    use ratatui::style::{Color, Modifier};

    let codes: Vec<&str> = codes_str.split(';').collect();
    let mut idx = 0;

    while idx < codes.len() {
        let code: u32 = codes[idx].parse().unwrap_or(0);
        match code {
            0 => style = ratatui::style::Style::default(),
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            7 => style = style.add_modifier(Modifier::REVERSED),
            9 => style = style.add_modifier(Modifier::CROSSED_OUT),
            22 => style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            30 => style = style.fg(Color::Black),
            31 => style = style.fg(Color::Red),
            32 => style = style.fg(Color::Green),
            33 => style = style.fg(Color::Yellow),
            34 => style = style.fg(Color::Blue),
            35 => style = style.fg(Color::Magenta),
            36 => style = style.fg(Color::Cyan),
            37 => style = style.fg(Color::White),
            39 => style = style.fg(Color::Reset),
            40 => style = style.bg(Color::Black),
            41 => style = style.bg(Color::Red),
            42 => style = style.bg(Color::Green),
            43 => style = style.bg(Color::Yellow),
            44 => style = style.bg(Color::Blue),
            45 => style = style.bg(Color::Magenta),
            46 => style = style.bg(Color::Cyan),
            47 => style = style.bg(Color::White),
            49 => style = style.bg(Color::Reset),
            90 => style = style.fg(Color::DarkGray),
            91 => style = style.fg(Color::LightRed),
            92 => style = style.fg(Color::LightGreen),
            93 => style = style.fg(Color::LightYellow),
            94 => style = style.fg(Color::LightBlue),
            95 => style = style.fg(Color::LightMagenta),
            96 => style = style.fg(Color::LightCyan),
            97 => style = style.fg(Color::Gray),
            38 => {
                if idx + 1 < codes.len() {
                    match codes[idx + 1] {
                        "5" if idx + 2 < codes.len() => {
                            let n: u8 = codes[idx + 2].parse().unwrap_or(0);
                            style = style.fg(Color::Indexed(n));
                            idx += 2;
                        }
                        "2" if idx + 4 < codes.len() => {
                            let r: u8 = codes[idx + 2].parse().unwrap_or(0);
                            let g: u8 = codes[idx + 3].parse().unwrap_or(0);
                            let b: u8 = codes[idx + 4].parse().unwrap_or(0);
                            style = style.fg(Color::Rgb(r, g, b));
                            idx += 4;
                        }
                        _ => {}
                    }
                }
            }
            48 => {
                if idx + 1 < codes.len() {
                    match codes[idx + 1] {
                        "5" if idx + 2 < codes.len() => {
                            let n: u8 = codes[idx + 2].parse().unwrap_or(0);
                            style = style.bg(Color::Indexed(n));
                            idx += 2;
                        }
                        "2" if idx + 4 < codes.len() => {
                            let r: u8 = codes[idx + 2].parse().unwrap_or(0);
                            let g: u8 = codes[idx + 3].parse().unwrap_or(0);
                            let b: u8 = codes[idx + 4].parse().unwrap_or(0);
                            style = style.bg(Color::Rgb(r, g, b));
                            idx += 4;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    style
}

// ── Treesitter-enhanced rendering ─────────────────────────────────────────────

/// A raw ANSI-parsed span: the text content and the SGR-decoded style, plus
/// the char-column offset at which this span starts within the plain-text line.
#[derive(Debug, Clone)]
struct RawSpan {
    text: String,
    style: Style,
    /// Byte offset from the start of the PLAIN-TEXT line (ANSI stripped).
    col_start: usize,
}

/// Metadata for one display line in the difftastic side-by-side output.
/// Used to map display lines back to positions in the reconstructed source files.
#[derive(Debug, Clone)]
struct DisplayLineInfo {
    /// Index of the old-source line this display-line's LEFT side belongs to.
    /// `None` if the left side is a `.` placeholder (no old-file content).
    left_src_line: Option<usize>,
    /// Byte offset within `old_source[left_src_line]` at which this display row starts.
    /// Non-zero for wrapped (continuation) lines.
    left_byte_offset: usize,
    /// Same as above but for the RIGHT (new-file) side.
    right_src_line: Option<usize>,
    right_byte_offset: usize,
    /// Character-column at which the right side starts in the plain-text display line.
    split_col: usize,
}

/// Determine the column offset at which the RIGHT side of a difftastic
/// side-by-side display starts.
///
/// Difftastic always begins each side with a line-number label rendered in one
/// of the following SGR styles:
///   * `\x1b[2m`     – dim  (context / unchanged lines)
///   * `\x1b[92;1m`  – bright green + bold  (added lines)
///   * `\x1b[91;1m`  – bright red + bold    (removed lines)
///
/// The label text is a decimal line number or `.` followed by a space
/// (e.g. `"1 "`, `"42 "`, `". "`).
///
/// We scan the display lines for one that has TWO such labels; the column
/// offset of the second label is the split column.
fn find_split_col(raw_input: &str) -> Option<usize> {
    for line in raw_input.lines() {
        let col = find_split_col_in_line(line);
        if col.is_some() {
            return col;
        }
    }
    None
}

/// Returns the split column for a single raw (ANSI-containing) display line,
/// or `None` if the line doesn't have two line-number segments.
fn find_split_col_in_line(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut plain_col: usize = 0; // running plain-text column offset
    let mut linenum_count = 0usize;
    let mut second_linenum_col = 0usize;

    // State machine: track current SGR codes and accumulate plain-text length.
    let mut current_codes = String::new();

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            // Parse escape sequence.
            let seq_start = i + 2;
            let mut seq_end = seq_start;
            while seq_end < len && !bytes[seq_end].is_ascii_alphabetic() {
                seq_end += 1;
            }
            if seq_end < len && bytes[seq_end] == b'm' {
                current_codes = line[seq_start..seq_end].to_string();
            }
            i = seq_end + 1;
        } else {
            // Plain text byte — collect until the next escape or end.
            let text_start = i;
            while i < len && !(bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[') {
                i += 1;
            }
            let text = &line[text_start..i];

            // Check whether this segment is a line-number label.
            if is_linenum_color(&current_codes) && is_linenum_text(text) {
                linenum_count += 1;
                if linenum_count == 2 {
                    second_linenum_col = plain_col;
                }
            }

            plain_col += text.chars().count();
        }
    }

    if linenum_count >= 2 {
        Some(second_linenum_col)
    } else {
        None
    }
}

/// Returns `true` if the SGR codes represent one of the colors difftastic
/// uses for line-number labels.
fn is_linenum_color(codes: &str) -> bool {
    matches!(codes, "2" | "92;1" | "91;1" | "1")
}

/// Returns `true` if the text looks like a difftastic line-number label.
/// Valid forms: `"N "` (one or more digits + space) or `". "` (dot + space).
fn is_linenum_text(text: &str) -> bool {
    // Must end with a single space and the rest must be digits or a lone dot.
    if let Some(stripped) = text.strip_suffix(' ') {
        !stripped.is_empty() && (stripped == "." || stripped.chars().all(|c| c.is_ascii_digit()))
    } else {
        false
    }
}

// ── Source reconstruction ─────────────────────────────────────────────────────

/// Parse all raw ANSI spans in a single display line into `RawSpan`s,
/// tracking the plain-text column offset for each span.
fn parse_raw_spans(line: &str) -> Vec<RawSpan> {
    let mut spans: Vec<RawSpan> = Vec::new();
    let mut current_style = Style::default();
    let mut text_start = 0;
    let mut plain_col: usize = 0;
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            if i > text_start {
                let text = line[text_start..i].to_string();
                let col_start = plain_col - text.chars().count();
                spans.push(RawSpan {
                    text,
                    style: current_style,
                    col_start,
                });
            }

            let seq_start = i + 2;
            let mut seq_end = seq_start;
            while seq_end < len && !bytes[seq_end].is_ascii_alphabetic() {
                seq_end += 1;
            }

            if seq_end < len && bytes[seq_end] == b'm' {
                let codes_str = &line[seq_start..seq_end];
                current_style = apply_sgr_codes(current_style, codes_str);
            }

            i = seq_end + 1;
            text_start = i;
        } else {
            // Count plain-text chars as we go.
            let ch_len = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            plain_col += 1;
            i += ch_len;
        }
    }

    if text_start < len {
        let text = line[text_start..].to_string();
        let col_start = plain_col - text.chars().count();
        spans.push(RawSpan {
            text,
            style: current_style,
            col_start,
        });
    }

    if spans.is_empty() {
        spans.push(RawSpan {
            text: String::new(),
            style: Style::default(),
            col_start: 0,
        });
    }

    spans
}

/// Reconstruct the old-file and new-file source strings from a block of
/// difftastic side-by-side ANSI output, and return per-display-line metadata
/// that maps each display row back to a position in the reconstructed sources.
///
/// Difftastic wraps long source lines across multiple display rows.  A
/// continuation row is identified by a `.` line-number label on the relevant
/// side; its content is appended directly to the previous source line (no
/// newline inserted between them).
fn reconstruct_sources(
    raw_input: &str,
    split_col: usize,
) -> (String, String, Vec<DisplayLineInfo>) {
    let mut old_lines: Vec<String> = Vec::new(); // reconstructed old-file source lines
    let mut new_lines: Vec<String> = Vec::new(); // reconstructed new-file source lines

    // Per-display-line metadata.
    let mut display_infos: Vec<DisplayLineInfo> = Vec::new();

    // Track the byte offset within the CURRENT source line for continuation rows.
    // Reset to 0 each time a new source line starts.
    let mut old_current_byte_offset: usize = 0;
    let mut new_current_byte_offset: usize = 0;

    for line in raw_input.lines() {
        let spans = parse_raw_spans(line);

        // Collect line-number labels and the text content for each side.
        // We split spans into left-side (col < split_col) and right-side (col >= split_col).
        let mut left_linenum: Option<&str> = None; // "N" or "."
        let mut right_linenum: Option<&str> = None;
        let mut left_content = String::new();
        let mut right_content = String::new();
        let mut saw_left_linenum = false;
        let mut saw_right_linenum = false;

        for span in &spans {
            let on_right = span.col_start >= split_col;

            if is_linenum_color(&sgr_codes_for_style(span.style)) && is_linenum_text(&span.text) {
                // This span is a line-number label.
                if !on_right && !saw_left_linenum {
                    left_linenum = Some(span.text.trim());
                    saw_left_linenum = true;
                } else if on_right && !saw_right_linenum {
                    right_linenum = Some(span.text.trim());
                    saw_right_linenum = true;
                }
            } else if saw_left_linenum && !on_right {
                // Content on the left side (after the line-number label).
                left_content.push_str(&span.text);
            } else if saw_right_linenum && on_right {
                // Content on the right side (after the line-number label).
                right_content.push_str(&span.text);
            }
        }

        // The left side is always padded by difftastic to fill the fixed column width.
        // Strip that trailing whitespace so the reconstructed source is clean.
        let left_content = left_content.trim_end().to_string();

        // Determine this display row's relationship to the source lines.
        //
        // A `.` label means:
        //   LEFT `.`  → either (a) no old-file content for this row (pure addition),
        //                       or (b) continuation of the previous old-file source line.
        //   RIGHT `.` → same logic for new-file.
        //
        // We distinguish (a) from (b) by whether the OTHER side also has a `.` label:
        //   Both `.` → continuation of the LAST source line for the side that has
        //              non-empty content.  (difft wraps the side that has content.)
        //   One side has a real number, other is `.` → pure insertion/deletion.

        // --- LEFT side ---
        let left_src_line: Option<usize>;
        let left_byte_offset: usize;

        match left_linenum {
            Some(".") => {
                // Either a placeholder (pure addition) or a continuation.
                // It's a continuation when LEFT has non-empty content on this row.
                if !left_content.is_empty() {
                    // Continuation: append to the last old-source line.
                    if let Some(last) = old_lines.last_mut() {
                        let offset = old_current_byte_offset;
                        last.push_str(&left_content);
                        old_current_byte_offset += left_content.len();
                        left_src_line = Some(old_lines.len() - 1);
                        left_byte_offset = offset;
                    } else {
                        left_src_line = None;
                        left_byte_offset = 0;
                    }
                } else {
                    // Pure placeholder — no old-file content here.
                    left_src_line = None;
                    left_byte_offset = 0;
                    old_current_byte_offset = 0; // reset (next real left line is fresh)
                }
            }
            Some(num) if num.chars().all(|c| c.is_ascii_digit()) => {
                // New source line.
                old_lines.push(left_content.clone());
                left_src_line = Some(old_lines.len() - 1);
                left_byte_offset = 0;
                old_current_byte_offset = left_content.len();
            }
            _ => {
                // No left-side label at all (e.g. header line).
                left_src_line = None;
                left_byte_offset = 0;
            }
        }

        // --- RIGHT side ---
        let right_src_line: Option<usize>;
        let right_byte_offset: usize;

        match right_linenum {
            Some(".") => {
                if !right_content.is_empty() {
                    if let Some(last) = new_lines.last_mut() {
                        let offset = new_current_byte_offset;
                        last.push_str(&right_content);
                        new_current_byte_offset += right_content.len();
                        right_src_line = Some(new_lines.len() - 1);
                        right_byte_offset = offset;
                    } else {
                        right_src_line = None;
                        right_byte_offset = 0;
                    }
                } else {
                    right_src_line = None;
                    right_byte_offset = 0;
                    new_current_byte_offset = 0;
                }
            }
            Some(num) if num.chars().all(|c| c.is_ascii_digit()) => {
                new_lines.push(right_content.clone());
                right_src_line = Some(new_lines.len() - 1);
                right_byte_offset = 0;
                new_current_byte_offset = right_content.len();
            }
            _ => {
                right_src_line = None;
                right_byte_offset = 0;
            }
        }

        display_infos.push(DisplayLineInfo {
            left_src_line,
            left_byte_offset,
            right_src_line,
            right_byte_offset,
            split_col,
        });
    }

    let old_source = old_lines.join("\n");
    let new_source = new_lines.join("\n");
    (old_source, new_source, display_infos)
}

/// Convert a ratatui `Style` back into the SGR codes string that would produce it,
/// just enough to feed `is_linenum_color`.  We only need to distinguish the four
/// line-number color codes that difftastic uses.
fn sgr_codes_for_style(style: Style) -> String {
    use ratatui::style::{Color, Modifier};
    let bold = style.add_modifier == Modifier::BOLD || style.add_modifier.contains(Modifier::BOLD);
    let dim = style.add_modifier.contains(Modifier::DIM);

    match style.fg {
        Some(Color::LightGreen) if bold => "92;1".to_string(),
        Some(Color::LightRed) if bold => "91;1".to_string(),
        _ if dim => "2".to_string(),
        _ if bold => "1".to_string(),
        _ => String::new(),
    }
}

// ── Main syntax-enhanced entry point ─────────────────────────────────────────

/// Parse difftastic ANSI output into ratatui `Line`s, overlaying treesitter
/// syntax highlighting.
///
/// For each token in the output:
/// * If difftastic colored it green/red (added/removed), that fg color is
///   converted to a background color and the treesitter fg color is applied
///   on top, so you get syntax variety even inside large added/removed blocks.
/// * For unchanged (context) tokens, treesitter fg is applied directly.
/// * Dim-colored line-number labels are left untouched.
///
/// Falls back to plain `parse_ansi_to_lines` when:
/// * No treesitter config exists for the file extension.
/// * The split column cannot be detected (e.g. binary-diff output).
/// * Treesitter returns no highlight data.
pub fn parse_ansi_to_lines_with_syntax(
    raw_input: &str,
    filename: &str,
    hl: &SyntaxHighlighter,
    t: &Theme,
) -> Vec<ratatui::text::Line<'static>> {
    // Fast path: detect the side-by-side split column.
    // If we can't find it (e.g. binary diff, header-only output), fall back.
    let Some(split_col) = find_split_col(raw_input) else {
        return parse_ansi_to_lines(raw_input);
    };

    // Reconstruct the old and new source files from the display output.
    let (old_source, new_source, display_infos) = reconstruct_sources(raw_input, split_col);

    // Run treesitter over both reconstructed sources.
    // highlight() returns None for unknown file extensions — fall back in that case.
    let old_hl = hl.highlight(filename, &old_source);
    let new_hl = hl.highlight(filename, &new_source);

    // If treesitter failed for both sides, fall back.
    if old_hl.is_none() && new_hl.is_none() {
        return parse_ansi_to_lines(raw_input);
    }

    let old_hl = old_hl.unwrap_or_default();
    let new_hl = new_hl.unwrap_or_default();

    // Rebuild each display line using treesitter colors.
    let mut result: Vec<ratatui::text::Line<'static>> = Vec::new();

    for (raw_line, info) in raw_input.lines().zip(display_infos.iter()) {
        let raw_spans = parse_raw_spans(raw_line);
        let mut new_spans: Vec<ratatui::text::Span<'static>> = Vec::new();

        // Track the running byte offset within the source line for each side.
        // We accumulate the byte lengths of all content spans seen so far.
        let mut left_content_bytes: usize = 0;
        let mut right_content_bytes: usize = 0;

        for span in &raw_spans {
            if span.text.is_empty() {
                continue;
            }

            let on_right = span.col_start >= info.split_col;
            let is_label =
                is_linenum_color(&sgr_codes_for_style(span.style)) && is_linenum_text(&span.text);

            // Compute the byte range of this span within its source line.
            // Byte offset = (accumulated content bytes on this side) + (side's base byte offset).
            let (src_line_idx, span_byte_start, hl_data) = if on_right {
                if is_label {
                    (info.right_src_line, 0usize, &new_hl)
                } else {
                    let start = info.right_byte_offset + right_content_bytes;
                    (info.right_src_line, start, &new_hl)
                }
            } else if is_label {
                (info.left_src_line, 0usize, &old_hl)
            } else {
                let start = info.left_byte_offset + left_content_bytes;
                (info.left_src_line, start, &old_hl)
            };
            let span_byte_end = span_byte_start + span.text.len();

            // Advance content byte counter (only for non-label spans).
            if !is_label {
                if on_right {
                    right_content_bytes += span.text.len();
                } else {
                    left_content_bytes += span.text.len();
                }
            }

            // Look up treesitter highlight for this span's byte range.
            let hl_fg: Option<Color> = if !is_label {
                src_line_idx
                    .and_then(|sl| hl_data.get(sl))
                    .and_then(|hl_line| find_best_hl_span(hl_line, span_byte_start, span_byte_end))
                    .and_then(|h| h.hl_index)
                    .map(|idx| t.syntax_color(idx))
            } else {
                None
            };

            // Determine the change-status background color from difftastic's fg.
            let diff_bg = change_bg_from_style(span.style, t);

            let final_style = if is_label {
                // Keep line-number labels with their original dim/colored style.
                span.style
            } else {
                let fg = hl_fg.unwrap_or(t.text);
                let mut s = Style::default().fg(fg);
                if let Some(bg) = diff_bg {
                    s = s.bg(bg);
                }
                s
            };

            new_spans.push(ratatui::text::Span::styled(span.text.clone(), final_style));
        }

        if new_spans.is_empty() {
            new_spans.push(ratatui::text::Span::raw(String::new()));
        }

        result.push(ratatui::text::Line::from(new_spans));
    }

    result
}

/// Find the `HlSpan` that best covers the byte range `[start, end)` in a line.
/// Returns the span with the greatest overlap, or `None` if no spans touch the range.
fn find_best_hl_span(hl_line: &[HlSpan], start: usize, end: usize) -> Option<&HlSpan> {
    hl_line
        .iter()
        .filter(|s| s.end > start && s.start < end)
        .max_by_key(|s| {
            let overlap_start = s.start.max(start);
            let overlap_end = s.end.min(end);
            overlap_end.saturating_sub(overlap_start)
        })
}

/// Given the style of a difftastic span, return the corresponding diff
/// background color from the theme (for added / removed tokens), or `None`
/// for unchanged context tokens.
fn change_bg_from_style(style: Style, t: &Theme) -> Option<Color> {
    use ratatui::style::Color;
    match style.fg {
        Some(Color::LightGreen) => Some(t.diff_added_bg),
        Some(Color::Green) => Some(t.diff_added_bg),
        Some(Color::LightRed) => Some(t.diff_removed_bg),
        Some(Color::Red) => Some(t.diff_removed_bg),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal raw ANSI line that looks like one context (unchanged)
    /// display line from difft with two sides.
    ///
    /// Format: `\x1b[2mN \x1b[0mcontent_left  \x1b[2mN \x1b[0mcontent_right`
    fn ctx_line(left_no: u32, left: &str, right_no: u32, right: &str, pad: usize) -> String {
        let padded_left = format!("{}{}", left, " ".repeat(pad.saturating_sub(left.len())));
        format!("\x1b[2m{left_no} \x1b[0m{padded_left}\x1b[2m{right_no} \x1b[0m{right}")
    }

    /// Build an added-line display row: left is `. ` placeholder, right has green content.
    fn added_line(right_no: u32, right: &str, pad: usize) -> String {
        let padded_left = " ".repeat(pad + 2); // ". " plus spaces
        format!("\x1b[2m. \x1b[0m{padded_left}\x1b[92;1m{right_no} \x1b[0m\x1b[92m{right}\x1b[0m")
    }

    #[test]
    fn find_split_col_detects_context_line() {
        // A context line with left num at 0 and right num at col 41.
        let line = ctx_line(1, "fn main() {", 1, "fn main() {", 39);
        let input = format!("{line}\n");
        let col = find_split_col(&input);
        // Left linenum "1 " starts at col 0; right linenum "1 " starts after
        // left side's padded content.  Should be 41.
        assert!(col.is_some(), "should detect a split column");
        // The split col should be > 0.
        assert!(col.unwrap() > 0);
    }

    #[test]
    fn find_split_col_returns_none_for_header_only() {
        // A header line like "src/foo.rs --- Rust" has no two linenum labels.
        let input = "\x1b[1m\x1b[93msrc/foo.rs\x1b[0m\x1b[2m --- Rust\x1b[0m\n";
        assert!(find_split_col(input).is_none());
    }

    #[test]
    fn is_linenum_text_accepts_valid_labels() {
        assert!(is_linenum_text("1 "));
        assert!(is_linenum_text("42 "));
        assert!(is_linenum_text(". "));
        assert!(is_linenum_text("100 "));
    }

    #[test]
    fn is_linenum_text_rejects_invalid() {
        assert!(!is_linenum_text("fn"));
        assert!(!is_linenum_text("1")); // no trailing space
        assert!(!is_linenum_text(""));
        assert!(!is_linenum_text(" "));
        assert!(!is_linenum_text("let x = 1; ")); // too long / not pure digits
    }

    #[test]
    fn reconstruct_sources_context_lines() {
        // Two context lines, both sides identical.
        let pad = 39;
        let l1 = ctx_line(1, "fn main() {", 1, "fn main() {", pad);
        let l2 = ctx_line(2, "    let x = 1;", 2, "    let x = 1;", pad);
        let input = format!("{l1}\n{l2}\n");

        let split_col = find_split_col(&input).expect("should find split col");
        let (old_src, new_src, infos) = reconstruct_sources(&input, split_col);

        assert_eq!(old_src, "fn main() {\n    let x = 1;");
        assert_eq!(new_src, "fn main() {\n    let x = 1;");
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].left_src_line, Some(0));
        assert_eq!(infos[0].right_src_line, Some(0));
        assert_eq!(infos[1].left_src_line, Some(1));
        assert_eq!(infos[1].right_src_line, Some(1));
    }

    #[test]
    fn reconstruct_sources_pure_addition() {
        // Left side has `. ` placeholder; only right has content.
        let pad = 39;
        let ctx = ctx_line(1, "fn main() {", 1, "fn main() {", pad);
        let add = added_line(2, "    let y = 42;", pad);
        let input = format!("{ctx}\n{add}\n");

        let split_col = find_split_col(&input).expect("should find split col");
        let (old_src, new_src, infos) = reconstruct_sources(&input, split_col);

        // Old source only has the context line.
        assert_eq!(old_src, "fn main() {");
        // New source has context line + added line.
        assert_eq!(new_src, "fn main() {\n    let y = 42;");
        // Added display line: left has no src line, right has src line 1.
        assert_eq!(infos[1].left_src_line, None);
        assert_eq!(infos[1].right_src_line, Some(1));
    }

    #[test]
    fn parse_ansi_to_lines_roundtrip() {
        // Ensure the basic ANSI parser still works correctly (regression guard).
        let input = "\x1b[92;1m3 \x1b[0m    \x1b[92mlet y = 42;\x1b[0m";
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 1);
        // Spans should have the right text content.
        let texts: Vec<&str> = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(texts.contains(&"3 "), "line number span missing");
        assert!(texts.contains(&"let y = 42;"), "content span missing");
    }
}
