use crate::app::{App, LoadState};
use crate::ui::{
    theme::Theme,
    utils::{render_hint_bar, review_badge},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

// ── PR list column layout constants ──────────────────────────────────────────

/// Width of the PR number column (` #NNNNN`).
const COL_NUMBER_WIDTH: usize = 7;
/// Width of the stack tree connector prefix (e.g. `┬ `, `├ `, `└ `, `  `).
const COL_STACK_PREFIX_WIDTH: usize = 2;
/// Separator between number and title.
const COL_SEP1_WIDTH: usize = 1;
/// Indent before author name.
const COL_INDENT_WIDTH: usize = 2;
/// Width of the author name column.
const COL_AUTHOR_WIDTH: usize = 20;
/// Separator between author and stats.
const COL_SEP2_WIDTH: usize = 2;
/// Width of the `[DRAFT]` label when shown.
const COL_DRAFT_WIDTH: usize = 8;
/// Longest CI status badge (e.g. `"  ◌ PENDING"` = 11 chars).
const CI_COL_WIDTH: usize = 11;
/// Longest review decision badge (e.g. `"  ✓ APPROVED"` = 12 chars).
const REVIEW_COL_WIDTH: usize = 12;
/// Width of the stack size badge (e.g. `"  ≡3"` = 4 chars).
const STACK_BADGE_WIDTH: usize = 4;

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

/// Return the display order for the PR list: stacked PRs are grouped together
/// (bottom → top), standalone PRs appear in their original order interspersed.
///
/// Returns a `Vec<usize>` of indices into `app.prs`.
/// Delegates to `App::pr_display_order()` for the logic (shared with input handler).
fn display_order(app: &App) -> Vec<usize> {
    app.pr_display_order()
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

    let order = display_order(app);

    // Map the cursor (index into app.prs) to the display position.
    let display_cursor = order
        .iter()
        .position(|&idx| idx == app.pr_cursor)
        .unwrap_or(0);

    let items: Vec<ListItem> = order
        .iter()
        .map(|&pr_idx| {
            let pr = &app.prs[pr_idx];

            // Stack tree connector and optional badge.
            let (stack_prefix, stack_badge_span) =
                if let Some(pos) = app.stack_positions.get(&pr.number) {
                    let connector = match (pos.below.is_some(), pos.above.is_some()) {
                        (false, true) => "┬ ",  // bottom of stack
                        (true, true) => "├ ",   // middle
                        (true, false) => "└ ",  // top
                        (false, false) => "  ", // single (shouldn't happen)
                    };
                    // Show stack size badge only on the bottom PR.
                    let badge = if pos.depth == 0 {
                        let s = format!(" ≡{}", pos.total);
                        Span::styled(
                            format!("{:<width$}", s, width = STACK_BADGE_WIDTH),
                            Style::default()
                                .fg(t.text_accent)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::raw(" ".repeat(STACK_BADGE_WIDTH))
                    };
                    (connector, badge)
                } else {
                    ("  ", Span::raw(" ".repeat(STACK_BADGE_WIDTH)))
                };

            let number = format!(" #{:<5}", pr.number);
            let author = format!("{:<width$}", pr.author, width = COL_AUTHOR_WIDTH);
            let additions = pr.additions.map(|v| format!("+{v}")).unwrap_or_default();
            let deletions = pr.deletions.map(|v| format!("-{v}")).unwrap_or_default();
            let total = match (pr.additions, pr.deletions) {
                (Some(a), Some(d)) => format!("±{}", a + d),
                _ => String::new(),
            };
            let stats = format!("{:>6} {:>6} {:>7}", additions, deletions, total);

            let inner_width = area.width.saturating_sub(2) as usize;
            let fixed = COL_STACK_PREFIX_WIDTH
                + COL_NUMBER_WIDTH
                + COL_SEP1_WIDTH
                + COL_INDENT_WIDTH
                + COL_AUTHOR_WIDTH
                + COL_SEP2_WIDTH
                + stats.len()
                + if pr.draft { COL_DRAFT_WIDTH } else { 0 }
                + CI_COL_WIDTH
                + REVIEW_COL_WIDTH
                + STACK_BADGE_WIDTH;
            let title_width = inner_width.saturating_sub(fixed);
            let title_display = truncate(&pr.title, title_width);

            let mut spans = vec![
                Span::styled(
                    stack_prefix,
                    Style::default().fg(t.text_dim).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    number,
                    Style::default()
                        .fg(t.pr_number)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<width$}", title_display, width = title_width),
                    Style::default().fg(t.text),
                ),
            ];

            if pr.draft {
                spans.push(Span::styled(
                    " ▸DRAFT",
                    Style::default().fg(t.pr_draft).add_modifier(Modifier::BOLD),
                ));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(author, Style::default().fg(t.pr_author)));
            spans.push(Span::raw("  "));
            spans.push(Span::styled(stats, Style::default().fg(t.text_dim)));

            // CI status badge — always renders a fixed-width span so columns align.
            let ci_span = if let Some(state) = pr.ci_status.as_deref() {
                let (text, color) = ci_badge(state);
                let padded = format!("{:<width$}", format!("  {text}"), width = CI_COL_WIDTH);
                Span::styled(
                    padded,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw(" ".repeat(CI_COL_WIDTH))
            };

            // Review decision badge — always renders a fixed-width span so columns align.
            let review_span = if let Some(decision) = pr.review_decision.as_deref() {
                let (text, color) = review_badge(decision);
                let padded = format!("{:<width$}", format!("  {text}"), width = REVIEW_COL_WIDTH);
                Span::styled(
                    padded,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw(" ".repeat(REVIEW_COL_WIDTH))
            };

            spans.push(ci_span);
            spans.push(review_span);
            spans.push(stack_badge_span);

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(t.border_dim_style())
                .style(t.background_style()),
        )
        .highlight_style(t.selection_style());

    let mut list_state = ListState::default();
    list_state.select(Some(display_cursor));

    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_statusbar(f: &mut Frame, area: Rect, t: &Theme) {
    let hints: &[(&str, &str)] = &[
        ("j/k", "navigate"),
        ("Enter", "open"),
        ("/", "search repo"),
        ("a", "toggle mine/all"),
        ("R", "review requests"),
        ("r", "refresh"),
        ("o", "browser"),
        ("T", "theme"),
        ("?", "help"),
        ("q", "quit"),
    ];
    render_hint_bar(f, hints, area, t);
}
