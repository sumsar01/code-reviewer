use crate::app::{App, LoadState, Screen};
use crossterm::event::KeyCode;

pub fn handle_key_review_requests(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
            app.screen = Screen::PrList;
        }
        KeyCode::Char('?') => {
            app.show_help = true;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.rr_prs.is_empty() {
                app.rr_cursor = (app.rr_cursor + 1).min(app.rr_prs.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.rr_cursor = app.rr_cursor.saturating_sub(1);
        }
        KeyCode::Enter => {
            if let Some(pr) = app.rr_prs.get(app.rr_cursor).cloned() {
                app.open_review_request_pr(pr.repo_owner, pr.repo_name, pr.number, pr.url);
            }
        }
        KeyCode::Char('a') => {
            app.rr_show_all = !app.rr_show_all;
            app.rr_cursor = 0;
            app.rr_load_state = LoadState::Loading;
            app.fetch_review_request_prs();
        }
        KeyCode::Char('o') => {
            if let Some(pr) = app.rr_prs.get(app.rr_cursor) {
                let _ = open::that(&pr.url);
            }
        }
        KeyCode::Char('r') => {
            app.rr_cursor = 0;
            app.rr_load_state = LoadState::Loading;
            app.fetch_review_request_prs();
        }
        _ => {}
    }
    false
}
