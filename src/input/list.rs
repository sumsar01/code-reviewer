use crate::app::{App, LoadState, RepoSwitcherState};
use crate::ui::theme::Theme;
use crossterm::event::KeyCode;

pub fn handle_key_list(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.prs.is_empty() {
                app.pr_cursor = (app.pr_cursor + 1).min(app.prs.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.pr_cursor = app.pr_cursor.saturating_sub(1);
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
