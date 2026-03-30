use crate::app::{App, LoadState};
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Truncate a string to `max_chars`, appending `…` if truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        chars[..max_chars.saturating_sub(1)]
            .iter()
            .collect::<String>()
            + "…"
    }
}

/// Return a short badge text and color for a review decision string.
fn review_badge(decision: &str) -> (&'static str, Color) {
    match decision {
        "APPROVED" => ("✓ APPROVED", Color::Green),
        "CHANGES_REQUESTED" => ("✗ CHANGES", Color::Red),
        "REVIEW_REQUIRED" => ("? REVIEW", Color::Yellow),
        _ => ("? REVIEW", Color::Yellow),
    }
}

/// Return a short badge text and color for a CI status rollup state.
fn ci_badge(state: &str) -> (&'static str, Color) {
    match state {
        "SUCCESS" => ("● PASS", Color::Green),
        "FAILURE" => ("✗ FAIL", Color::Red),
        "ERROR" => ("✗ ERROR", Color::Red),
        "PENDING" => ("◌ PENDING", Color::Yellow),
        _ => ("◌ PENDING", Color::Yellow),
    }
}

pub fn render(f: &mut Frame, app: &mut App, t: &Theme) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Min(0),    // list
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_header(f, app, chunks[0], t);
    render_list(f, app, chunks[1], t);
    render_statusbar(f, chunks[2], t);
}

fn render_header(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let repo = app
        .repo
        .as_ref()
        .map(|r| r.full_name())
        .unwrap_or_else(|| "no repo".to_string());

    let filter = if app.config.ui.show_all_prs {
        "all PRs"
    } else {
        "my PRs"
    };

    let line = Line::from(vec![
        Span::styled(
            " prr ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(repo, Style::default().fg(t.text_dim)),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("[{filter}]"),
            Style::default()
                .fg(t.pr_number)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let p = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::NONE)
            .style(t.background_style()),
    );
    f.render_widget(p, area);
}

fn render_list(f: &mut Frame, app: &mut App, area: Rect, t: &Theme) {
    let block = Block::default()
        .borders(Borders::NONE)
        .style(t.background_style());

    match &app.pr_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading pull requests…")
                .style(t.text_dim_style())
                .block(block);
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let error_detail = e.clone();
            let text = Text::from(vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "Could not load pull requests",
                        Style::default()
                            .fg(t.diff_removed_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(error_detail, t.text_dim_style()),
                ]),
            ]);
            let p = Paragraph::new(text).block(block);
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.prs.is_empty() {
        let repo = app
            .repo
            .as_ref()
            .map(|r| r.full_name())
            .unwrap_or_else(|| "this repository".to_string());

        let text = if app.config.ui.show_all_prs {
            Text::from(vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("No open pull requests in {repo}"),
                        Style::default()
                            .fg(t.text_accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Press 'r' to refresh", t.text_dim_style()),
                ]),
            ])
        } else {
            Text::from(vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("No open pull requests from you in {repo}"),
                        Style::default()
                            .fg(t.text_accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "Press 'a' to show all pull requests, or 'r' to refresh",
                        t.text_dim_style(),
                    ),
                ]),
            ])
        };

        let p = Paragraph::new(text).block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .prs
        .iter()
        .enumerate()
        .map(|(i, pr)| {
            let selected = i == app.pr_cursor;
            let base_style = if selected {
                t.selection_style()
            } else {
                Style::default()
            };

            let number = format!(" #{:<5}", pr.number);
            let author = format!("{:<20}", pr.author);
            let additions = pr.additions.map(|v| format!("+{v}")).unwrap_or_default();
            let deletions = pr.deletions.map(|v| format!("-{v}")).unwrap_or_default();
            let total = match (pr.additions, pr.deletions) {
                (Some(a), Some(d)) => format!("±{}", a + d),
                _ => String::new(),
            };
            let stats = format!("{:>6} {:>6} {:>7}", additions, deletions, total);

            // Fixed column widths for badge columns — constant regardless of whether
            // data is present, so all rows align at the same horizontal positions.
            // Longest CI badge:     "  ◌ PENDING"  = 11 chars
            // Longest review badge: "  ✓ APPROVED" = 12 chars
            const CI_COL_WIDTH: usize = 11;
            const REVIEW_COL_WIDTH: usize = 12;

            let inner_width = area.width.saturating_sub(2) as usize;
            let fixed = 7
                + 1
                + 2
                + 20
                + 2
                + stats.len()
                + if pr.draft { 8 } else { 0 }
                + CI_COL_WIDTH
                + REVIEW_COL_WIDTH;
            let title_width = inner_width.saturating_sub(fixed);
            let title_display = truncate(&pr.title, title_width);

            let mut spans = vec![
                Span::styled(
                    number,
                    base_style.fg(t.pr_number).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", base_style),
                Span::styled(
                    format!("{:<width$}", title_display, width = title_width),
                    base_style.fg(t.text),
                ),
            ];

            if pr.draft {
                spans.push(Span::styled(
                    " ▸DRAFT",
                    base_style.fg(t.pr_draft).add_modifier(Modifier::BOLD),
                ));
            }

            spans.push(Span::styled("  ", base_style));
            spans.push(Span::styled(author, base_style.fg(t.pr_author)));
            spans.push(Span::styled("  ", base_style));
            spans.push(Span::styled(stats, base_style.fg(t.text_dim)));

            // CI status badge — always renders a fixed-width span so columns align.
            let ci_span = if let Some(state) = pr.ci_status.as_deref() {
                let (text, color) = ci_badge(state);
                let padded = format!("{:<width$}", format!("  {text}"), width = CI_COL_WIDTH);
                Span::styled(padded, base_style.fg(color).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(" ".repeat(CI_COL_WIDTH), base_style)
            };

            // Review decision badge — always renders a fixed-width span so columns align.
            let review_span = if let Some(decision) = pr.review_decision.as_deref() {
                let (text, color) = review_badge(decision);
                let padded = format!("{:<width$}", format!("  {text}"), width = REVIEW_COL_WIDTH);
                Span::styled(padded, base_style.fg(color).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(" ".repeat(REVIEW_COL_WIDTH), base_style)
            };

            spans.push(ci_span);
            spans.push(review_span);

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(t.border_dim_style())
            .style(t.background_style()),
    );
    f.render_widget(list, area);
}

fn render_statusbar(f: &mut Frame, area: Rect, t: &Theme) {
    let hints: &[(&str, &str)] = &[
        ("j/k", "navigate"),
        ("Enter", "open"),
        ("a", "toggle mine/all"),
        ("r", "refresh"),
        ("o", "browser"),
        ("T", "theme"),
        ("?", "help"),
        ("q", "quit"),
    ];

    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", t.text_dim_style()));
        }
        spans.push(Span::styled(format!(" {key} "), t.key_badge_style()));
        spans.push(Span::styled(format!(" {desc}"), t.key_desc_style()));
    }

    let p = Paragraph::new(Line::from(spans)).style(t.background_style());
    f.render_widget(p, area);
}
