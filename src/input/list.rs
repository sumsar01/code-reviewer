use crate::app::{App, LoadState, RepoSwitcherState, Screen};
use crate::ui::theme::Theme;
use crossterm::event::KeyCode;

pub fn handle_key_list(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.prs.is_empty() {
                let order = app.pr_display_order();
                if let Some(display_pos) = order.iter().position(|&idx| idx == app.pr_cursor) {
                    let next_pos = (display_pos + 1).min(order.len() - 1);
                    app.pr_cursor = order[next_pos];
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !app.prs.is_empty() {
                let order = app.pr_display_order();
                if let Some(display_pos) = order.iter().position(|&idx| idx == app.pr_cursor) {
                    let prev_pos = display_pos.saturating_sub(1);
                    app.pr_cursor = order[prev_pos];
                }
            }
        }
        KeyCode::Enter => {
            if !app.prs.is_empty() {
                app.open_detail();
            }
        }
        KeyCode::Char('a') => {
            app.config.ui.show_all_prs = !app.config.ui.show_all_prs;
            app.pr_cursor = 0;
            app.pr_load_state = LoadState::Loading;
            app.fetch_prs();
        }
        KeyCode::Char('r') => {
            app.pr_cursor = 0;
            app.pr_load_state = LoadState::Loading;
            app.fetch_prs();
        }
        KeyCode::Char('R') => {
            // Switch to the cross-repo review requests screen.
            app.screen = Screen::ReviewRequests;
            // Fetch only if we haven't already loaded (avoids redundant API calls).
            if app.rr_prs.is_empty() && !matches!(app.rr_load_state, LoadState::Loading) {
                app.rr_cursor = 0;
                app.rr_load_state = LoadState::Loading;
                app.fetch_review_request_prs();
            }
        }
        KeyCode::Char('o') => {
            if let Some(pr) = app.prs.get(app.pr_cursor) {
                let _ = open::that(&pr.url);
            }
        }
        KeyCode::Char('T') => {
            app.theme_picker_original = Some(app.theme.clone());
            app.theme_picker_cursor = Theme::index_of(&app.config.ui.theme);
            app.show_theme_picker = true;
        }
        KeyCode::Char('/') => {
            let recent = app.config.ui.recent_repos.clone();
            app.repo_switcher = Some(RepoSwitcherState::new(recent));
        }
        _ => {}
    }
    false
}
