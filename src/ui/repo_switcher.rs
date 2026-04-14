use crate::app::{App, RepoSwitcherState, SearchState};
use crate::ui::{theme::Theme, utils::render_hint_bar};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, t: &Theme) {
    let area = switcher_rect(f.area());
    f.render_widget(Clear, area);

    let state = match &app.repo_switcher {
        Some(s) => s,
        None => return,
    };

    // How many suggestion rows we'll display.
    let suggestions = visible_suggestions(state);
    // Each suggestion takes 1 line (name) + 1 if it has a description.
    let suggestion_lines: u16 = suggestions
        .iter()
        .map(|(_, desc)| if desc.is_some() { 2 } else { 1 })
        .sum::<u16>()
        .max(1);
    // Total inner content height: 1 (input) + 1 (divider) + suggestion_lines + 1 (padding)
    let inner_height = 3 + suggestion_lines;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(inner_height), Constraint::Length(1)])
        .split(area);

    // ── Main box ─────────────────────────────────────────────────────────────
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_style())
        .style(t.background_style())
        .title(Span::styled(
            " Search repos ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = outer_block.inner(chunks[0]);
    f.render_widget(outer_block, chunks[0]);

    // Split inner into input row + suggestion rows
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // query input
            Constraint::Length(1), // divider / hint
            Constraint::Min(0),    // suggestions
        ])
        .split(inner);

    // ── Query input line ─────────────────────────────────────────────────────
    let input_line = Line::from(vec![Span::styled(
        format!(" ❯ {}▌", state.query),
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )]);
    f.render_widget(
        Paragraph::new(input_line).style(t.background_style()),
        inner_chunks[0],
    );

    // ── Divider / status hint ─────────────────────────────────────────────────
    let divider_text = match &state.search_state {
        SearchState::Idle => {
            if state.query.is_empty() {
                " recent repos "
            } else {
                " searching… "
            }
        }
        SearchState::Loading => " searching… ",
        SearchState::Done => " results ",
        SearchState::Error(_) => " error ",
    };
    let divider_line = Line::from(Span::styled(divider_text, t.text_dim_style()));
    f.render_widget(
        Paragraph::new(divider_line).style(t.background_style()),
        inner_chunks[1],
    );

    // ── Suggestions ───────────────────────────────────────────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();

    if let SearchState::Error(msg) = &state.search_state {
        lines.push(Line::from(Span::styled(
            format!("  ✗ {}", msg),
            Style::default().fg(t.diff_removed_fg),
        )));
    } else if suggestions.is_empty() && matches!(state.search_state, SearchState::Done) {
        lines.push(Line::from(Span::styled(
            "  No repositories found",
            t.text_dim_style(),
        )));
    } else {
        for (i, (label, desc)) in suggestions.iter().enumerate() {
            let is_selected = i == state.cursor;
            let cursor_glyph = if is_selected { "▸ " } else { "  " };

            let line = if is_selected {
                Line::from(Span::styled(
                    format!(" {cursor_glyph}{label}"),
                    Style::default()
                        .fg(t.tab_active)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(
                    format!(" {cursor_glyph}{label}"),
                    t.text_dim_style(),
                ))
            };
            lines.push(line);

            // Description subtitle (dimmed, truncated to fit width)
            if let Some(d) = desc {
                let max_w = area.width.saturating_sub(6) as usize;
                // Use Unicode-aware truncation so multi-byte characters (e.g.
                // Chinese, emoji) don't cause a panic from a mid-codepoint byte slice.
                let truncated = truncate_str(&d, max_w);
                lines.push(Line::from(Span::styled(
                    format!("    {truncated}"),
                    t.text_dim_style(),
                )));
            }
        }
    }

    f.render_widget(
        Paragraph::new(lines).style(t.background_style()),
        inner_chunks[2],
    );

    // ── Status bar ────────────────────────────────────────────────────────────
    let hints: &[(&str, &str)] = &[
        ("type", "search"),
        ("↑/↓", "navigate"),
        ("Tab", "fill"),
        ("Enter", "open"),
        ("Ctrl+W", "del word"),
        ("Esc", "cancel"),
    ];
    render_hint_bar(f, hints, chunks[1], t);
}

/// Return the list of (display_label, optional_description) pairs to show.
fn visible_suggestions(state: &RepoSwitcherState) -> Vec<(String, Option<String>)> {
    if state.query.is_empty() {
        // Show recent repos when query is empty — no description available
        state
            .recent_repos
            .iter()
            .map(|r| (r.clone(), None))
            .collect()
    } else {
        state
            .results
            .iter()
            .map(|r| {
                let base = format!("{}/{}", r.owner, r.name);
                let label = if r.stars > 0 {
                    format!("{}  ★ {}", base, format_stars(r.stars))
                } else {
                    base
                };
                (label, r.description.clone())
            })
            .collect()
    }
}

/// Truncate `s` to at most `max_display_cols` terminal columns, appending "…"
/// when the string is shortened.  Uses Unicode character count as a proxy for
/// display width (good enough for CJK; a full wcwidth implementation would
/// require an extra dep).  Crucially, it never slices mid-codepoint.
fn truncate_str(s: &str, max_display_cols: usize) -> String {
    if max_display_cols == 0 {
        return String::new();
    }
    let mut col = 0usize;
    let mut last_safe_byte = 0usize;
    for (byte_pos, ch) in s.char_indices() {
        let ch_width = unicode_display_width(ch);
        if col + ch_width > max_display_cols.saturating_sub(1) {
            // Would overflow — truncate here and append ellipsis.
            return format!("{}…", &s[..last_safe_byte]);
        }
        col += ch_width;
        last_safe_byte = byte_pos + ch.len_utf8();
    }
    // String fits entirely.
    s.to_string()
}

/// Returns the approximate terminal display width for a single Unicode character.
/// Wide characters (CJK Unified Ideographs, Hangul, fullwidth forms, etc.) return 2;
/// combining/zero-width characters return 0; everything else returns 1.
fn unicode_display_width(ch: char) -> usize {
    let cp = ch as u32;
    if cp == 0 {
        return 0;
    }
    // Combining / zero-width ranges
    if (0x0300..=0x036F).contains(&cp) {
        return 0;
    }
    if (0x1DC0..=0x1DFF).contains(&cp) {
        return 0;
    }
    if (0x20D0..=0x20FF).contains(&cp) {
        return 0;
    }
    if (0xFE20..=0xFE2F).contains(&cp) {
        return 0;
    }
    // Wide ranges
    if (0x1100..=0x115F).contains(&cp) {
        return 2;
    } // Hangul Jamo
    if (0x2E80..=0x303E).contains(&cp) {
        return 2;
    } // CJK Radicals / misc
    if (0x3040..=0x33FF).contains(&cp) {
        return 2;
    } // Japanese / CJK compat
    if (0x3400..=0x4DBF).contains(&cp) {
        return 2;
    } // CJK Extension A
    if (0x4E00..=0x9FFF).contains(&cp) {
        return 2;
    } // CJK Unified Ideographs
    if (0xA000..=0xA4CF).contains(&cp) {
        return 2;
    } // Yi
    if (0xA960..=0xA97F).contains(&cp) {
        return 2;
    } // Hangul Jamo Extended-A
    if (0xAC00..=0xD7FF).contains(&cp) {
        return 2;
    } // Hangul Syllables
    if (0xF900..=0xFAFF).contains(&cp) {
        return 2;
    } // CJK Compat Ideographs
    if (0xFE10..=0xFE1F).contains(&cp) {
        return 2;
    } // Vertical forms
    if (0xFE30..=0xFE4F).contains(&cp) {
        return 2;
    } // CJK Compat Forms
    if (0xFF01..=0xFF60).contains(&cp) {
        return 2;
    } // Fullwidth
    if (0xFFE0..=0xFFE6).contains(&cp) {
        return 2;
    } // Fullwidth signs
    if cp >= 0x1B000 {
        return 2;
    } // Emoji / supplemental CJK
    1
}

fn format_stars(n: u32) -> String {
    if n >= 1_000 {
        format!("{:.1}k", n as f32 / 1000.0)
    } else {
        n.to_string()
    }
}

/// Compute the overlay rect: 60 cols wide, up to 20 rows tall, centered.
fn switcher_rect(r: Rect) -> Rect {
    let width = 60u16.min(r.width.saturating_sub(4));
    let height = 20u16.min(r.height.saturating_sub(4));

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
