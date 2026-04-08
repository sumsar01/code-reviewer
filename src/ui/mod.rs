pub mod comment_peek;
pub mod comments;
pub mod constants;
pub mod diff;
pub mod difftastic;
pub mod file_tree;
pub mod help;
pub mod pr_detail;
pub mod pr_list;
pub mod repo_switcher;
pub mod review_input;
pub mod review_requests;
pub mod theme;
pub mod theme_picker;
pub mod update_prompt;
pub mod utils;

use crate::app::{App, Screen};
use ratatui::Frame;

pub fn render(f: &mut Frame, app: &mut App) {
    let t = app.theme.clone();

    match &app.screen {
        Screen::PrList => pr_list::render(f, app, &t),
        Screen::PrDetail => pr_detail::render(f, app, &t),
        Screen::ReviewRequests => review_requests::render(f, app, &t),
    }

    if app.show_help {
        help::render(f, &t);
    }

    if app.show_theme_picker {
        theme_picker::render(f, app, &t);
    }

    if app.review_overlay.is_some() {
        review_input::render(f, app, &t);
    }

    if app.comment_peek.is_some() {
        comment_peek::render(f, app, &t);
    }

    if app.update_available.is_some() {
        update_prompt::render(f, app, &t);
    }

    if app.repo_switcher.is_some() {
        repo_switcher::render(f, app, &t);
    }
}
