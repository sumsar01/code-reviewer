use crate::app::{App, LoadState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Split: title bar | list | status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(0),    // list
            Constraint::Length(1), // status
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_list(f, app, chunks[1]);
    render_statusbar(f, app, chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
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

    let title = format!(" prr  {repo}  [{filter}] ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, area);
}

fn render_list(f: &mut Frame, app: &mut App, area: Rect) {
    match &app.pr_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("Loading pull requests…")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("Error: {e}"))
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.prs.is_empty() {
        let msg = if app.config.ui.show_all_prs {
            "No open PRs for this repository."
        } else {
            "No open PRs from you. Press 'a' to show all."
        };
        let p = Paragraph::new(msg).block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .prs
        .iter()
        .enumerate()
        .map(|(i, pr)| {
            let selected = i == app.pr_cursor;
            let draft_tag = if pr.draft { " [draft]" } else { "" };
            let number = format!("#{:<5}", pr.number);
            let author = format!("{:<20}", pr.author);
            let additions = pr.additions.map(|v| format!("+{v}")).unwrap_or_default();
            let deletions = pr.deletions.map(|v| format!("-{v}")).unwrap_or_default();
            let stats = format!("{:>6} {:>6}", additions, deletions);

            let style = if selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(number, style.fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(format!("{}{}", pr.title, draft_tag), style),
                Span::raw("  "),
                Span::styled(author, style.fg(Color::Blue)),
                Span::styled(format!("  {}", stats), style.fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL));
    f.render_widget(list, area);
}

fn render_statusbar(f: &mut Frame, _app: &App, area: Rect) {
    let help = Paragraph::new(
        " j/k navigate  Enter open  a toggle mine/all  r refresh  o browser  ? help  q quit",
    )
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, area);
}
