use crate::app::{App, LoadState};
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
/// Separator between number and title.
const COL_SEP_WIDTH: usize = 1;
/// Width of the author column.
const COL_AUTHOR_WIDTH: usize = 20;

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

    let line = Line::from(vec![
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

    let p = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::NONE)
            .style(t.background_style()),
    );
    f.render_widget(p, area);
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

    let items: Vec<ListItem> = app
        .rr_prs
        .iter()
        .map(|pr| {
            let repo = truncate(
                &format!("{}/{}", pr.repo_owner, pr.repo_name),
                COL_REPO_WIDTH,
            );
            let repo_col = format!("{:<width$}", repo, width = COL_REPO_WIDTH);
            let number = format!(" #{:<5}", pr.number);
            let author = format!("{:<width$}", pr.author, width = COL_AUTHOR_WIDTH);

            let inner_width = area.width.saturating_sub(2) as usize;
            let fixed = COL_REPO_WIDTH
                + COL_SEP_WIDTH
                + COL_NUMBER_WIDTH
                + COL_SEP_WIDTH
                + COL_AUTHOR_WIDTH
                + 2;
            let title_width = inner_width.saturating_sub(fixed);
            let title_display = truncate(&pr.title, title_width);

            let mut spans = vec![
                Span::styled(" ", Style::default()),
                Span::styled(repo_col, Style::default().fg(t.text_dim)),
                Span::styled(
                    number,
                    Style::default()
                        .fg(t.pr_number)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
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

            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(author, Style::default().fg(t.pr_author)));

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
    list_state.select(Some(app.rr_cursor));

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
