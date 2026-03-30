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
                let truncated = if d.len() > max_w {
                    format!("{}…", &d[..max_w.saturating_sub(1)])
                } else {
                    d.clone()
                };
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
        ("j/k", "navigate"),
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
