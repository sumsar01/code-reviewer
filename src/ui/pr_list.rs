use crate::app::{App, LoadState};
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
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
    match &app.pr_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading pull requests…")
                .style(t.text_dim_style())
                .block(
                    Block::default()
                        .borders(Borders::NONE)
                        .style(t.background_style()),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("  Error: {e}"))
                .style(Style::default().fg(t.diff_removed_fg))
                .block(
                    Block::default()
                        .borders(Borders::NONE)
                        .style(t.background_style()),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.prs.is_empty() {
        let msg = if app.config.ui.show_all_prs {
            "  No open PRs for this repository."
        } else {
            "  No open PRs from you. Press 'a' to show all."
        };
        let p = Paragraph::new(msg).style(t.text_dim_style()).block(
            Block::default()
                .borders(Borders::NONE)
                .style(t.background_style()),
        );
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

            // Badge text for review decision (empty string when none)
            let badge_text = pr
                .review_decision
                .as_deref()
                .map(|d| {
                    let (text, _) = review_badge(d);
                    format!("  {text}")
                })
                .unwrap_or_default();

            let inner_width = area.width.saturating_sub(2) as usize;
            let fixed =
                7 + 1 + 2 + 20 + 2 + stats.len() + if pr.draft { 8 } else { 0 } + badge_text.len();
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

            // Append review decision badge if present
            if let Some(decision) = pr.review_decision.as_deref() {
                let (text, color) = review_badge(decision);
                spans.push(Span::styled(
                    format!("  {text}"),
                    base_style.fg(color).add_modifier(Modifier::BOLD),
                ));
            }

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
