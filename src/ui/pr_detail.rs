use crate::app::{App, DetailTab};
use crate::syntax::SyntaxHighlighter;
use crate::ui::{comments, diff, difftastic, theme::Theme};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, t: &Theme) {
    let pr = match app.prs.get(app.pr_cursor) {
        Some(pr) => pr.clone(),
        None => return,
    };

    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // PR header
            Constraint::Length(2), // tabs (slim)
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_pr_header(f, &pr, chunks[0], t);
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

    render_statusbar(f, chunks[3], t);
}

fn render_pr_header(f: &mut Frame, pr: &crate::github::PullRequest, area: Rect, t: &Theme) {
    let draft = if pr.draft { "  ▸DRAFT" } else { "" };

    let lines = vec![
        // Title line
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
        // Author + branch line
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
        // Stats line
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

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_style())
            .style(t.background_style())
            .title(Span::styled(
                " Pull Request ",
                Style::default().fg(t.text_dim),
            )),
    );
    f.render_widget(p, area);
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

fn render_statusbar(f: &mut Frame, area: Rect, t: &Theme) {
    let hints: &[(&str, &str)] = &[
        ("Tab", "focus tree/content"),
        ("Space", "toggle tree"),
        ("j/k", "scroll"),
        ("n/N", "next/prev file"),
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
