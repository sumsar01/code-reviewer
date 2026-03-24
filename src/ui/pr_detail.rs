use crate::app::{App, DetailTab};
use crate::ui::{comments, diff};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let pr = match app.prs.get(app.pr_cursor) {
        Some(pr) => pr.clone(),
        None => return,
    };

    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // PR header
            Constraint::Length(3), // tabs
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_pr_header(f, &pr, chunks[0]);
    render_tabs(f, app, chunks[1]);

    match app.detail_tab {
        DetailTab::Diff => diff::render(f, app, chunks[2]),
        DetailTab::Comments => comments::render(f, app, chunks[2]),
    }

    render_statusbar(f, app, chunks[3]);
}

fn render_pr_header(f: &mut Frame, pr: &crate::github::PullRequest, area: Rect) {
    let draft = if pr.draft { " [DRAFT]" } else { "" };
    let stats = match (pr.additions, pr.deletions, pr.changed_files) {
        (Some(a), Some(d), Some(c)) => format!("+{a} -{d}  {c} files changed"),
        _ => String::new(),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("#{} ", pr.number),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}{}", pr.title, draft),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("author: ", Style::default().fg(Color::DarkGray)),
            Span::styled(pr.author.clone(), Style::default().fg(Color::Blue)),
            Span::raw("  "),
            Span::styled(
                format!("{} → {}", pr.head_branch, pr.base_branch),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(stats, Style::default().fg(Color::Green))),
    ];

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Pull Request "),
    );
    f.render_widget(p, area);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = vec![Line::from("Diff"), Line::from("Comments")];

    let selected = match app.detail_tab {
        DetailTab::Diff => 0,
        DetailTab::Comments => 1,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn render_statusbar(f: &mut Frame, _app: &App, area: Rect) {
    let p = Paragraph::new(
        " Tab switch pane  j/k scroll  n/N next/prev file  c checkout  o browser  Esc back  ? help",
    )
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(p, area);
}
