use crate::app::{App, DetailTab, LoadState};
use crate::github::{CheckRun, PullRequest};
use crate::ui::{
    comments, diff, difftastic,
    theme::Theme,
    utils::{render_hint_bar, review_badge},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};
use std::sync::Arc;

/// Ordered list of (pr_number, head_branch) entries for all PRs in a stack,
/// plus the index of the currently displayed PR within that list.
type StackInfo = (Vec<(u64, String)>, usize);

/// Build stack context for the currently displayed PR.
///
/// Returns `None` when the PR is standalone (not in a stack).
fn build_stack_info(app: &App, pr: &PullRequest) -> Option<StackInfo> {
    app.stack_positions.get(&pr.number)?;

    // Walk down to the bottom of the stack.
    let mut bottom_number = pr.number;
    while let Some(below) = app
        .stack_positions
        .get(&bottom_number)
        .and_then(|p| p.below)
    {
        bottom_number = below;
    }

    // Walk up from the bottom, collecting (number, head_branch) in order.
    let number_to_pr: std::collections::HashMap<u64, &PullRequest> =
        app.prs.iter().map(|p| (p.number, p)).collect();

    let mut chain: Vec<(u64, String)> = Vec::new();
    let mut current = bottom_number;
    loop {
        if let Some(p) = number_to_pr.get(&current) {
            chain.push((p.number, p.head_branch.clone()));
        }
        match app.stack_positions.get(&current).and_then(|p| p.above) {
            Some(next) => current = next,
            None => break,
        }
    }

    let current_idx = chain.iter().position(|(n, _)| *n == pr.number)?;
    Some((chain, current_idx))
}

pub fn render(f: &mut Frame, app: &mut App, t: &Theme) {
    let pr = match app.prs.get(app.pr_cursor) {
        Some(pr) => pr.clone(),
        None => return,
    };

    let area = f.area();
    let (reviewed, total) = app.reviewed_progress();

    // Build stack navigator info (None for standalone PRs).
    let stack_info = build_stack_info(app, &pr);
    let stack_line = u16::from(stack_info.is_some());

    let has_reviewed_line = total > 0;
    let has_decision_line = pr.review_decision.is_some();
    let check_runs_lines = check_runs_line_count(&app.check_runs_load_state, &app.check_runs);
    // Layout: first (3 + stack_line) lines span full width.
    // Below that, left col = reviewed + decision, right col = CI checks.
    // Height = (3 + stack_line) fixed + max(left optional lines, right CI lines) + 2 borders.
    let left_optional = u16::from(has_reviewed_line) + u16::from(has_decision_line);
    let header_height = 2 + 3 + stack_line + left_optional.max(check_runs_lines);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height), // PR header
            Constraint::Length(2),             // tabs (slim)
            Constraint::Min(0),                // content
            Constraint::Length(1),             // status bar
        ])
        .split(area);

    render_pr_header(
        f,
        &pr,
        chunks[0],
        t,
        (reviewed, total),
        &app.check_runs_load_state,
        &app.check_runs,
        stack_info.as_ref(),
    );
    render_tabs(f, app, chunks[1], t);

    // Clone the Arc (cheap reference-count bump) before the mutable borrow of
    // `app` that happens inside diff::render / difftastic::render.
    let hl = Arc::clone(&app.syntax_hl);

    match app.detail_tab {
        DetailTab::Diff => diff::render(f, app, chunks[2], t, &hl),
        DetailTab::Comments => comments::render(f, app, chunks[2], t),
        DetailTab::Difftastic => difftastic::render(f, app, chunks[2], t, &hl),
    }

    render_statusbar(f, app, chunks[3], t);
}

fn render_pr_header(
    f: &mut Frame,
    pr: &crate::github::PullRequest,
    area: Rect,
    t: &Theme,
    reviewed_progress: (usize, usize),
    check_runs_load_state: &LoadState,
    check_runs: &[CheckRun],
    stack_info: Option<&StackInfo>,
) {
    let draft = if pr.draft { "  ▸DRAFT" } else { "" };
    let (reviewed, total) = reviewed_progress;

    // ── Outer border block ────────────────────────────────────────────────────
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(t.border_style())
        .style(t.background_style())
        .title(Span::styled(
            " Pull Request ",
            Style::default().fg(t.text_dim),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let stack_line = u16::from(stack_info.is_some());
    let top_height = 3 + stack_line;

    // ── Top lines: title / author / stats [/ stack] (full width) ─────────────
    let mut top_lines = vec![
        Line::from(vec![
            Span::styled(
                format!("#{} ", pr.number),
                Style::default()
                    .fg(t.pr_number)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}{}", pr.title, draft),
                Style::default().fg(t.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("by ", Style::default().fg(t.text_dim)),
            Span::styled(
                pr.author.clone(),
                Style::default()
                    .fg(t.pr_author)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled(pr.head_branch.clone(), Style::default().fg(t.text_accent)),
            Span::styled(" → ", Style::default().fg(t.text_dim)),
            Span::styled(pr.base_branch.clone(), Style::default().fg(t.text_dim)),
        ]),
        Line::from(match (pr.additions, pr.deletions, pr.changed_files) {
            (Some(a), Some(d), Some(c)) => vec![
                Span::styled(format!("+{a}"), Style::default().fg(t.stats_added)),
                Span::styled("  ", Style::default()),
                Span::styled(format!("-{d}"), Style::default().fg(t.stats_removed)),
                Span::styled(
                    format!("  {c} files changed"),
                    Style::default().fg(t.text_dim),
                ),
            ],
            _ => vec![],
        }),
    ];

    // Optional 4th line: Stack navigator.
    // Example: Stack:  [#121 auth-layer] → [#122 api-routes ★] → [#123 frontend]
    if let Some((chain, current_idx)) = stack_info {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled("Stack: ", Style::default().fg(t.text_dim)));
        for (i, (num, branch)) in chain.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" → ", Style::default().fg(t.text_dim)));
            }
            if i == *current_idx {
                spans.push(Span::styled(
                    format!("[#{num} {branch} ★]"),
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!("[#{num} {branch}]"),
                    Style::default().fg(t.text_dim),
                ));
            }
        }
        top_lines.push(Line::from(spans));
    }

    // Top section spans full inner width.
    let top_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: top_height.min(inner.height),
    };
    f.render_widget(
        Paragraph::new(top_lines).style(t.background_style()),
        top_area,
    );

    // Nothing left to render below the top section.
    if inner.height <= top_height {
        return;
    }

    // ── Bottom section: left col (reviewed/decision) | right col (CI) ────────
    let bottom_area = Rect {
        x: inner.x,
        y: inner.y + top_height,
        width: inner.width,
        height: inner.height - top_height,
    };

    let has_ci = !matches!(check_runs_load_state, LoadState::Idle) || !check_runs.is_empty();

    if has_ci {
        // Split horizontally: left 55% | right 45%
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(bottom_area);

        f.render_widget(
            Paragraph::new(reviewed_decision_lines(pr, reviewed, total, t))
                .style(t.background_style()),
            cols[0],
        );

        // Right col: CI checks
        let mut ci_lines: Vec<Line> = Vec::new();
        match check_runs_load_state {
            LoadState::Loading => {
                ci_lines.push(Line::from(vec![
                    Span::styled("CI: ", Style::default().fg(t.text_dim)),
                    Span::styled("loading…", Style::default().fg(t.text_dim)),
                ]));
            }
            LoadState::Idle if !check_runs.is_empty() => {
                ci_lines.push(Line::from(Span::styled(
                    "CI Checks:",
                    Style::default().fg(t.text_dim),
                )));
                for run in check_runs {
                    let (icon, color) = check_run_icon(run);
                    let conclusion_label = run
                        .conclusion
                        .as_deref()
                        .unwrap_or(run.status.as_str())
                        .to_lowercase();
                    let duration =
                        format_duration(run.started_at.as_deref(), run.completed_at.as_deref());
                    ci_lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {icon}  "),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(run.name.clone(), Style::default().fg(t.text)),
                        Span::styled(format!("  {conclusion_label}"), Style::default().fg(color)),
                        Span::styled(format!("  {duration}"), Style::default().fg(t.text_dim)),
                    ]));
                }
            }
            LoadState::Error(e) => {
                ci_lines.push(Line::from(vec![
                    Span::styled("CI: ", Style::default().fg(t.text_dim)),
                    Span::styled(format!("error: {e}"), Style::default().fg(Color::Red)),
                ]));
            }
            _ => {}
        }
        f.render_widget(
            Paragraph::new(ci_lines).style(t.background_style()),
            cols[1],
        );
    } else {
        // No CI data at all — single column for reviewed/decision only.
        f.render_widget(
            Paragraph::new(reviewed_decision_lines(pr, reviewed, total, t))
                .style(t.background_style()),
            bottom_area,
        );
    }
}

/// Return (icon, color) for a check run based on status + conclusion.
fn check_run_icon(run: &CheckRun) -> (&'static str, Color) {
    match run.status.as_str() {
        "COMPLETED" => match run.conclusion.as_deref().unwrap_or("") {
            "SUCCESS" | "SKIPPED" | "NEUTRAL" => ("✓", Color::Green),
            "FAILURE" | "TIMED_OUT" | "ACTION_REQUIRED" => ("✗", Color::Red),
            "CANCELLED" => ("✗", Color::DarkGray),
            _ => ("◌", Color::DarkGray),
        },
        "IN_PROGRESS" => ("↻", Color::Yellow),
        _ => ("◌", Color::DarkGray), // QUEUED or unknown
    }
}

/// Build the left-column lines: reviewed-progress (if any) + review decision (if any).
fn reviewed_decision_lines<'a>(
    pr: &crate::github::PullRequest,
    reviewed: usize,
    total: usize,
    t: &'a Theme,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    if total > 0 {
        let progress_color = if reviewed == total {
            ratatui::style::Color::Green
        } else {
            t.text_dim
        };
        lines.push(Line::from(vec![
            Span::styled("Reviewed: ", Style::default().fg(t.text_dim)),
            Span::styled(
                format!("{reviewed} / {total} files"),
                Style::default()
                    .fg(progress_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    if let Some(decision) = pr.review_decision.as_deref() {
        let (text, color) = review_badge(decision);
        lines.push(Line::from(vec![
            Span::styled("Review: ", Style::default().fg(t.text_dim)),
            Span::styled(
                text,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines
}

/// Compute "Xm Ys" duration from ISO-8601 strings, or "—" if unavailable.
///
/// Parses the full date+time so cross-midnight runs are handled correctly.
/// Expected format: "2024-01-15T10:23:45Z" (fractional seconds are stripped).
fn format_duration(started: Option<&str>, completed: Option<&str>) -> String {
    let (Some(s), Some(c)) = (started, completed) else {
        return "—".to_string();
    };

    /// Parse an ISO-8601 timestamp into total seconds since the Unix epoch
    /// (date portion * 86400 + time-of-day seconds).  We use a simple
    /// Gregorian day-count so we avoid pulling in `chrono` for this one helper.
    fn parse_epoch_secs(ts: &str) -> Option<i64> {
        // Split on 'T': ["2024-01-15", "10:23:45Z"]
        let mut parts = ts.split('T');
        let date_part = parts.next()?;
        let time_part = parts.next()?;

        let mut date_fields = date_part.split('-');
        let year: i64 = date_fields.next()?.parse().ok()?;
        let month: i64 = date_fields.next()?.parse().ok()?;
        let day: i64 = date_fields.next()?.parse().ok()?;

        let hms: Vec<&str> = time_part.trim_end_matches('Z').split(':').collect();
        if hms.len() < 3 {
            return None;
        }
        let h: i64 = hms[0].parse().ok()?;
        let m: i64 = hms[1].parse().ok()?;
        let sec: i64 = hms[2].split('.').next()?.parse().ok()?;

        // Days since epoch via a simple Gregorian formula (no leap-second handling needed).
        // Algorithm: count days in years 1970..year, plus days in months, plus day-1.
        let y = year;
        let mo = month;
        let d = day;
        let leap = |yr: i64| (yr % 4 == 0 && yr % 100 != 0) || yr % 400 == 0;
        let days_in_month = [0i64, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut days: i64 = (1970..y).map(|yr| if leap(yr) { 366 } else { 365 }).sum();
        for mi in 1..mo {
            days += days_in_month[mi as usize];
            if mi == 2 && leap(y) {
                days += 1;
            }
        }
        days += d - 1;

        Some(days * 86400 + h * 3600 + m * 60 + sec)
    }

    if let (Some(s_secs), Some(c_secs)) = (parse_epoch_secs(s), parse_epoch_secs(c)) {
        let elapsed = (c_secs - s_secs).abs();
        let mins = elapsed / 60;
        let secs = elapsed % 60;
        return format!("{mins}m {secs:02}s");
    }
    "—".to_string()
}

/// Number of lines the CI checks section will occupy in the header.
fn check_runs_line_count(state: &LoadState, runs: &[CheckRun]) -> u16 {
    match state {
        LoadState::Loading => 1,
        LoadState::Idle if !runs.is_empty() => 1 + runs.len() as u16, // header line + one per run
        LoadState::Error(_) => 1,
        _ => 0,
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    let tab_names = ["  Diff  ", "  Comments  ", "  Difftastic  "];

    let selected = match app.detail_tab {
        DetailTab::Diff => 0,
        DetailTab::Comments => 1,
        DetailTab::Difftastic => 2,
    };

    let titles: Vec<Line> = tab_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            if i == selected {
                Line::from(Span::styled(*name, t.tab_active_style()))
            } else {
                Line::from(Span::styled(*name, t.tab_inactive_style()))
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(t.border_dim_style())
                .style(t.background_style()),
        )
        .highlight_style(t.tab_active_style())
        .divider(Span::styled(" │ ", t.border_dim_style()));

    f.render_widget(tabs, area);
}

fn render_statusbar(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    // If there's a transient status message (e.g. after submitting a review),
    // show it instead of the normal key-hint bar.
    if let Some((msg, _)) = &app.status_message {
        use ratatui::style::Color;
        let p = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                msg.as_str().to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(t.background_style());
        f.render_widget(p, area);
        return;
    }

    let hints: &[(&str, &str)] = &[
        ("Tab", "focus tree/content"),
        ("Space", "toggle tree"),
        ("j/k", "scroll"),
        ("n/N", "next/prev file"),
        ("[/]", "stack prev/next"),
        ("v", "mark reviewed"),
        ("A/R/C", "approve/request/comment"),
        ("c", "checkout"),
        ("o", "browser"),
        ("T", "theme"),
        ("Esc", "back"),
        ("?", "help"),
    ];

    render_hint_bar(f, hints, area, t);
}
