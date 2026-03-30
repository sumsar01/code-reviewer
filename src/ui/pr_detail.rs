use crate::app::{App, DetailTab, LoadState};
use crate::github::CheckRun;
use crate::syntax::SyntaxHighlighter;
use crate::ui::{comments, diff, difftastic, theme::Theme};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

/// Return badge text and color for a review decision string.
fn review_badge(decision: &str) -> (&'static str, Color) {
    match decision {
        "APPROVED" => ("✓ APPROVED", Color::Green),
        "CHANGES_REQUESTED" => ("✗ CHANGES REQUESTED", Color::Red),
        "REVIEW_REQUIRED" => ("? REVIEW REQUIRED", Color::Yellow),
        _ => ("? REVIEW REQUIRED", Color::Yellow),
    }
}

pub fn render(f: &mut Frame, app: &mut App, t: &Theme) {
    let pr = match app.prs.get(app.pr_cursor) {
        Some(pr) => pr.clone(),
        None => return,
    };

    let area = f.area();
    let (reviewed, total) = app.reviewed_progress();

    let has_reviewed_line = total > 0;
    let has_decision_line = pr.review_decision.is_some();
    let check_runs_lines = check_runs_line_count(&app.check_runs_load_state, &app.check_runs);
    // Layout: first 3 lines (title/author/stats) span full width.
    // Below that, left col = reviewed + decision, right col = CI checks.
    // Height = 3 fixed + max(left optional lines, right CI lines) + 2 borders.
    let left_optional = u16::from(has_reviewed_line) + u16::from(has_decision_line);
    let header_height = 2 + 3 + left_optional.max(check_runs_lines);

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
    );
    render_tabs(f, app, chunks[1], t);

    // Extract a reference to the syntax highlighter *before* the mutable borrow
    // of `app` that happens inside diff::render.  We use a raw pointer to work
    // around the borrow checker; this is safe because `syntax_hl` is not
    // mutated by any code path below.
    let hl_ptr: *const SyntaxHighlighter = &app.syntax_hl;
    let hl: &SyntaxHighlighter = unsafe { &*hl_ptr };

    match app.detail_tab {
        DetailTab::Diff => diff::render(f, app, chunks[2], t, hl),
        DetailTab::Comments => comments::render(f, app, chunks[2], t),
        DetailTab::Difftastic => difftastic::render(f, app, chunks[2], t, hl),
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

    // ── Top 3 lines: title / author / stats (full width) ─────────────────────
    let top_lines = vec![
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

    // Top section spans full inner width, exactly 3 rows tall.
    let top_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 3.min(inner.height),
    };
    f.render_widget(
        Paragraph::new(top_lines).style(t.background_style()),
        top_area,
    );

    // Nothing left to render below the top section.
    if inner.height <= 3 {
        return;
    }

    // ── Bottom section: left col (reviewed/decision) | right col (CI) ────────
    let bottom_area = Rect {
        x: inner.x,
        y: inner.y + 3,
        width: inner.width,
        height: inner.height - 3,
    };

    let has_ci = !matches!(check_runs_load_state, LoadState::Idle) || !check_runs.is_empty();

    if has_ci {
        // Split horizontally: left 55% | right 45%
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(bottom_area);

        // Left col: reviewed progress + review decision
        let mut left_lines: Vec<Line> = Vec::new();
        if total > 0 {
            let progress_color = if reviewed == total {
                Color::Green
            } else {
                t.text_dim
            };
            left_lines.push(Line::from(vec![
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
            left_lines.push(Line::from(vec![
                Span::styled("Review: ", Style::default().fg(t.text_dim)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        f.render_widget(
            Paragraph::new(left_lines).style(t.background_style()),
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
        let mut left_lines: Vec<Line> = Vec::new();
        if total > 0 {
            let progress_color = if reviewed == total {
                Color::Green
            } else {
                t.text_dim
            };
            left_lines.push(Line::from(vec![
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
            left_lines.push(Line::from(vec![
                Span::styled("Review: ", Style::default().fg(t.text_dim)),
                Span::styled(
                    text,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        f.render_widget(
            Paragraph::new(left_lines).style(t.background_style()),
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

/// Compute "Xm Ys" duration from ISO-8601 strings, or "—" if unavailable.
fn format_duration(started: Option<&str>, completed: Option<&str>) -> String {
    let (Some(s), Some(c)) = (started, completed) else {
        return "—".to_string();
    };
    // Parse only the time portion for a quick heuristic — we just need seconds.
    // Full ISO-8601: "2024-01-15T10:23:45Z"
    let parse_secs = |ts: &str| -> Option<i64> {
        // Use a naive parse: split on 'T', then parse HH:MM:SS
        let time_part = ts.split('T').nth(1)?;
        let hms: Vec<&str> = time_part.trim_end_matches('Z').split(':').collect();
        if hms.len() < 3 {
            return None;
        }
        let h: i64 = hms[0].parse().ok()?;
        let m: i64 = hms[1].parse().ok()?;
        let s: i64 = hms[2].split('.').next()?.parse().ok()?;
        Some(h * 3600 + m * 60 + s)
    };
    if let (Some(s_secs), Some(c_secs)) = (parse_secs(s), parse_secs(c)) {
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
    if let Some(msg) = &app.status_message {
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
        ("v", "mark reviewed"),
        ("A/R/C", "approve/request/comment"),
        ("c", "checkout"),
        ("o", "browser"),
        ("T", "theme"),
        ("Esc", "back"),
        ("?", "help"),
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
