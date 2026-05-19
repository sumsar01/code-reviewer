use crate::app::{App, LoadState};
use crate::github::{CiStatus, ReviewDecision, ReviewRequestPr};
use crate::ui::{theme::Theme, utils::render_hint_bar};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Max display width for the `owner/name` repo column.
const COL_REPO_WIDTH: usize = 22;
/// Width of the PR number column (` #NNNNN`).
const COL_NUMBER_WIDTH: usize = 7;
/// Width of the review-decision badge (e.g. `✓ Approved  `).
const COL_REVIEW_WIDTH: usize = 13;
/// Width of the CI status badge (e.g. `● pass  `).
const COL_CI_WIDTH: usize = 8;
/// Width of the updated-at column (e.g. `3d ago   `).
const COL_UPDATED_WIDTH: usize = 9;
/// Width of the author column.
const COL_AUTHOR_WIDTH: usize = 18;
/// Width of the ` ▸DRAFT` badge (including leading space).
const COL_DRAFT_WIDTH: usize = 7;
/// Gaps between columns (single space).
const COL_GAP: usize = 2;

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

/// An item in the flat display list — either a section header or a PR row.
enum DisplayItem<'a> {
    SectionHeader {
        label: &'static str,
        count: usize,
        dimmed: bool,
    },
    PrRow {
        pr: &'a ReviewRequestPr,
        /// 0 = requested list, 1 = reviewed list
        list_id: u8,
        pr_index: usize,
        dimmed: bool,
    },
}

/// Build a flat display list with section headers inserted.
/// `requested` are PRs needing review; `reviewed` are PRs already reviewed.
fn build_display_items<'a>(
    requested: &'a [ReviewRequestPr],
    reviewed: &'a [ReviewRequestPr],
) -> Vec<DisplayItem<'a>> {
    let needs: Vec<(usize, &ReviewRequestPr)> = requested
        .iter()
        .enumerate()
        .filter(|(_, pr)| {
            matches!(
                pr.review_decision,
                ReviewDecision::ReviewRequired | ReviewDecision::Unknown
            )
        })
        .collect();

    let waiting: Vec<(usize, &ReviewRequestPr)> = requested
        .iter()
        .enumerate()
        .filter(|(_, pr)| {
            matches!(
                pr.review_decision,
                ReviewDecision::Approved | ReviewDecision::ChangesRequested
            )
        })
        .collect();

    let mut items: Vec<DisplayItem> = Vec::new();

    items.push(DisplayItem::SectionHeader {
        label: "NEEDS YOUR REVIEW",
        count: needs.len(),
        dimmed: false,
    });
    for (idx, pr) in &needs {
        items.push(DisplayItem::PrRow {
            pr,
            list_id: 0,
            pr_index: *idx,
            dimmed: false,
        });
    }

    if !waiting.is_empty() {
        items.push(DisplayItem::SectionHeader {
            label: "WAITING FOR AUTHOR",
            count: waiting.len(),
            dimmed: true,
        });
        for (idx, pr) in &waiting {
            items.push(DisplayItem::PrRow {
                pr,
                list_id: 0,
                pr_index: *idx,
                dimmed: true,
            });
        }
    }

    if !reviewed.is_empty() {
        items.push(DisplayItem::SectionHeader {
            label: "REVIEWED BY YOU",
            count: reviewed.len(),
            dimmed: true,
        });
        for (idx, pr) in reviewed.iter().enumerate() {
            items.push(DisplayItem::PrRow {
                pr,
                list_id: 1,
                pr_index: idx,
                dimmed: true,
            });
        }
    }

    items
}

/// Find the display-list position for a given (list_id, pr_index) pair.
fn display_index_for_pr(display_items: &[DisplayItem], list_id: u8, pr_index: usize) -> Option<usize> {
    display_items.iter().position(|item| {
        matches!(item, DisplayItem::PrRow { list_id: lid, pr_index: idx, .. } if *lid == list_id && *idx == pr_index)
    })
}

pub fn render(f: &mut Frame, app: &App, t: &Theme) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Min(0),    // list
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    render_header(f, app, chunks[0], t);
    render_list(f, app, chunks[1], t);
    render_statusbar(f, app, chunks[2], t);
}

fn render_header(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let filter_label = if app.rr_show_all {
        "[all requests]"
    } else {
        "[direct requests]"
    };

    let left = Line::from(vec![
        Span::styled(
            " prr ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            filter_label,
            Style::default()
                .fg(t.pr_number)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("@{}", app.github.username),
            Style::default().fg(t.text_dim),
        ),
    ]);

    let right = Line::from(vec![Span::styled(
        format!("v{}  ", env!("CARGO_PKG_VERSION")),
        Style::default().fg(t.text_dim),
    )])
    .right_aligned();

    let p = Paragraph::new(left).block(
        Block::default()
            .borders(Borders::NONE)
            .style(t.background_style()),
    );
    f.render_widget(p, area);

    let version_area = Rect { height: 1, ..area };
    let p2 = Paragraph::new(right).block(
        Block::default()
            .borders(Borders::NONE)
            .style(t.background_style()),
    );
    f.render_widget(p2, version_area);
}

fn render_list(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let block = Block::default()
        .borders(Borders::NONE)
        .style(t.background_style());

    match &app.rr_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading review requests…")
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
                        "Could not load review requests",
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

    if app.rr_prs.is_empty() {
        let text = Text::from(vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "No review requests — you're all caught up!",
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Press 'r' to refresh, 'a' to toggle direct/all",
                    t.text_dim_style(),
                ),
            ]),
        ]);
        let p = Paragraph::new(text).block(block);
        f.render_widget(p, area);
        return;
    }

    // Fixed columns total (excluding title).
    let fixed_cols = 2  // leading indent under header (2 spaces)
        + COL_REPO_WIDTH + COL_GAP
        + COL_NUMBER_WIDTH + COL_GAP
        + COL_REVIEW_WIDTH + COL_GAP
        + COL_CI_WIDTH + COL_GAP
        + COL_UPDATED_WIDTH + COL_GAP
        + COL_AUTHOR_WIDTH;

    let display = build_display_items(&app.rr_prs, &app.rr_reviewed_prs);

    let items: Vec<ListItem> = display
        .iter()
        .map(|item| match item {
            DisplayItem::SectionHeader {
                label,
                count,
                dimmed,
            } => {
                let style = if *dimmed {
                    Style::default().fg(t.text_dim)
                } else {
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD)
                };
                ListItem::new(Line::from(vec![Span::styled(
                    format!(" ▼ {} ({})", label, count),
                    style,
                )]))
            }
            DisplayItem::PrRow { pr, dimmed, list_id: _, .. } => {
                let repo = truncate(
                    &format!("{}/{}", pr.repo_owner, pr.repo_name),
                    COL_REPO_WIDTH,
                );
                let repo_col = format!("{:<width$}", repo, width = COL_REPO_WIDTH);
                let number = format!("#{:<width$}", pr.number, width = COL_NUMBER_WIDTH - 1);
                let updated = format!("{:<width$}", pr.updated_at, width = COL_UPDATED_WIDTH);
                let author = format!("{:<width$}", pr.author, width = COL_AUTHOR_WIDTH);

                let inner_width = area.width.saturating_sub(2) as usize;
                let draft_extra = if pr.draft { COL_DRAFT_WIDTH + COL_GAP } else { 0 };
                let title_width =
                    inner_width.saturating_sub(fixed_cols + draft_extra + COL_GAP);
                let title_display = format!(
                    "{:<width$}",
                    truncate(&pr.title, title_width),
                    width = title_width
                );

                // Review decision badge
                let (review_text, review_style) = match &pr.review_decision {
                    ReviewDecision::Approved => (
                        format!("{:<width$}", "✓ Approved", width = COL_REVIEW_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default()
                                .fg(t.diff_added_fg)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    ReviewDecision::ChangesRequested => (
                        format!("{:<width$}", "✗ Changes", width = COL_REVIEW_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default()
                                .fg(t.diff_removed_fg)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    ReviewDecision::ReviewRequired | ReviewDecision::Unknown => (
                        format!("{:<width$}", "⏳ Waiting", width = COL_REVIEW_WIDTH),
                        Style::default()
                            .fg(t.pr_number)
                            .add_modifier(Modifier::BOLD),
                    ),
                };

                // CI status badge
                let (ci_text, ci_style) = match &pr.ci_status {
                    CiStatus::Success => (
                        format!("{:<width$}", "● pass", width = COL_CI_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default().fg(t.diff_added_fg)
                        },
                    ),
                    CiStatus::Failure => (
                        format!("{:<width$}", "✗ fail", width = COL_CI_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default().fg(t.diff_removed_fg)
                        },
                    ),
                    CiStatus::Pending => (
                        format!("{:<width$}", "○ pend", width = COL_CI_WIDTH),
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default().fg(t.pr_number)
                        },
                    ),
                    CiStatus::Unknown => (
                        format!("{:<width$}", "– –", width = COL_CI_WIDTH),
                        Style::default().fg(t.text_dim),
                    ),
                };

                let dim_text_style = if *dimmed {
                    Style::default().fg(t.text_dim)
                } else {
                    Style::default().fg(t.text)
                };

                let mut spans = vec![
                    Span::styled("  ", Style::default()), // indent under section header
                    Span::styled(repo_col, Style::default().fg(t.text_dim)),
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        number,
                        if *dimmed {
                            Style::default().fg(t.text_dim)
                        } else {
                            Style::default()
                                .fg(t.pr_number)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(review_text, review_style),
                    Span::styled("  ", Style::default()),
                    Span::styled(ci_text, ci_style),
                    Span::styled("  ", Style::default()),
                    Span::styled(updated, Style::default().fg(t.text_dim)),
                    Span::styled("  ", Style::default()),
                    Span::styled(title_display, dim_text_style),
                ];

                if pr.draft {
                    spans.push(Span::styled("  ", Style::default()));
                    spans.push(Span::styled(
                        "▸DRAFT",
                        Style::default()
                            .fg(t.pr_draft)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                spans.push(Span::styled("  ", Style::default()));
                spans.push(Span::styled(
                    author,
                    Style::default().fg(t.pr_author),
                ));

                ListItem::new(Line::from(spans))
            }
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
    // Map rr_cursor (pr index) → display position (which skips section headers)
    let display_pos = display_index_for_pr(&display, 0, app.rr_cursor).unwrap_or(1);
    list_state.select(Some(display_pos));

    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_statusbar(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let filter_hint = if app.rr_show_all {
        "direct only"
    } else {
        "show all"
    };
    let hints: &[(&str, &str)] = &[
        ("j/k", "navigate"),
        ("Enter", "open PR"),
        ("a", filter_hint),
        ("o", "browser"),
        ("r", "refresh"),
        ("?", "help"),
        ("Esc", "back"),
    ];
    render_hint_bar(f, hints, area, t);
}
