use crate::config::{self, Config};
use crate::git::{self, RepoInfo};
use crate::github::{CheckRun, DiffFile, GitHubClient, PrMetadata, PullRequest, RepoSearchResult, ReviewComment, parse_diff};
use crate::syntax::SyntaxHighlighter;
use crate::ui;
use crate::ui::theme::Theme;
use crate::updater;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    collections::HashSet,
    io,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

// ── Timing / UI constants ─────────────────────────────────────────────────────

/// How long a status-bar message stays visible before being cleared.
const STATUS_MESSAGE_TTL: Duration = Duration::from_secs(3);

/// Poll interval for keyboard events, giving ~60 fps.
const TICK_RATE: Duration = Duration::from_millis(16);

/// Debounce delay before firing a repository search after the last keystroke.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// Maximum number of repository search results requested per query.
const SEARCH_PAGE_SIZE: u8 = 8;

/// Initial/fallback height used for the diff area before the first render.
const DEFAULT_DIFF_AREA_HEIGHT: u16 = 24;

// ── State enums ───────────────────────────────────────────────────────────────

/// Which kind of GitHub review to submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    Approve,
    RequestChanges,
    Comment,
    /// Inline comment on a specific diff line.
    InlineComment {
        /// File path relative to repo root.
        path: String,
        /// Absolute line number on the chosen side.
        line: u64,
        /// `"LEFT"` for removed lines, `"RIGHT"` for added/context lines.
        side: String,
    },
}

impl ReviewAction {
    pub fn title(&self) -> &'static str {
        match self {
            ReviewAction::Approve => "Approve PR",
            ReviewAction::RequestChanges => "Request Changes",
            ReviewAction::Comment => "Leave Comment",
            ReviewAction::InlineComment { .. } => "Inline Comment",
        }
    }

    pub fn event_str(&self) -> &'static str {
        match self {
            ReviewAction::Approve => "APPROVE",
            ReviewAction::RequestChanges => "REQUEST_CHANGES",
            ReviewAction::Comment => "COMMENT",
            ReviewAction::InlineComment { .. } => "COMMENT",
        }
    }

    /// Whether a non-empty body is required before submitting.
    pub fn body_required(&self) -> bool {
        matches!(
            self,
            ReviewAction::RequestChanges | ReviewAction::Comment | ReviewAction::InlineComment { .. }
        )
    }
}

/// State for the review-input overlay (multi-line text editor).
#[derive(Debug, Clone)]
pub struct ReviewOverlayState {
    pub action: ReviewAction,
    /// Lines of text in the input buffer.
    pub lines: Vec<String>,
    /// Index into `lines` of the cursor row.
    pub cursor_row: usize,
    /// Byte offset within `lines[cursor_row]` of the cursor column.
    pub cursor_col: usize,
    /// Non-empty when the overlay should display a validation error.
    pub error: Option<String>,
}

impl ReviewOverlayState {
    pub fn new(action: ReviewAction) -> Self {
        Self {
            action,
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            error: None,
        }
    }

    /// Return the body text as a single string (lines joined by `\n`).
    pub fn body(&self) -> String {
        self.lines.join("\n")
    }

    /// Insert a character at the current cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        self.lines[row].insert(col, ch);
        self.cursor_col += ch.len_utf8();
        self.error = None;
    }

    /// Insert a newline at the current cursor position (split the line).
    pub fn insert_newline(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let tail = self.lines[row].split_off(col);
        self.lines.insert(row + 1, tail);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.error = None;
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        if col > 0 {
            // Find the previous char boundary
            let line = &self.lines[row];
            let prev = line[..col]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.lines[row].remove(prev);
            self.cursor_col = prev;
        } else if row > 0 {
            // Merge this line into the previous one
            let current = self.lines.remove(row);
            let prev_len = self.lines[row - 1].len();
            self.lines[row - 1].push_str(&current);
            self.cursor_row -= 1;
            self.cursor_col = prev_len;
        }
        self.error = None;
    }

    /// Move cursor left one character.
    pub fn move_left(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        if col > 0 {
            let prev = self.lines[row][..col]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor_col = prev;
        } else if row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }
    }

    /// Move cursor right one character.
    pub fn move_right(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let line_len = self.lines[row].len();
        if col < line_len {
            let next = self.lines[row][col..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| col + i)
                .unwrap_or(line_len);
            self.cursor_col = next;
        } else if row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    /// Move cursor up one row, clamping column.
    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
        }
    }

    /// Move cursor down one row, clamping column.
    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    PrList,
    PrDetail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailTab {
    Diff,
    Comments,
    Difftastic,
}

/// Which panel inside the detail screen has keyboard focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailFocus {
    /// The diff / comments / difftastic content area.
    Content,
    /// The file-tree sidebar.
    FileTree,
}

#[derive(Debug, Clone)]
pub enum LoadState {
    Idle,
    Loading,
    Error(String),
}

/// State for the loading/result tracking inside the repo switcher.
#[derive(Debug, Clone)]
pub enum SearchState {
    /// Nothing happening (query empty / not yet triggered).
    Idle,
    /// A search request is in-flight.
    Loading,
    /// Results are ready (possibly empty).
    Done,
    /// The search API returned an error.
    Error(String),
}

/// All state for the repo-switcher overlay.
#[derive(Debug, Clone)]
pub struct RepoSwitcherState {
    /// The text the user has typed so far.
    pub query: String,
    /// Search results from the GitHub API.
    pub results: Vec<RepoSearchResult>,
    /// Which suggestion row is highlighted (0-indexed).
    pub cursor: usize,
    /// State of the background search.
    pub search_state: SearchState,
    /// Recent repos snapshot (copied from config when the overlay opens).
    pub recent_repos: Vec<String>,
    /// Debounce: time of the last query change. We fire a search 300 ms after
    /// the user stops typing.
    pub last_keystroke: Option<Instant>,
}

impl RepoSwitcherState {
    pub fn new(recent_repos: Vec<String>) -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            cursor: 0,
            search_state: SearchState::Idle,
            recent_repos,
            last_keystroke: None,
        }
    }

    /// Return the `owner/name` string for the currently highlighted suggestion,
    /// or `None` if there's nothing to select.
    pub fn selected_full_name(&self) -> Option<String> {
        if self.query.is_empty() {
            self.recent_repos.get(self.cursor).cloned()
        } else {
            let r = self.results.get(self.cursor)?;
            Some(format!("{}/{}", r.owner, r.name))
        }
    }

    /// Number of currently visible suggestion rows.
    pub fn suggestion_count(&self) -> usize {
        if self.query.is_empty() {
            self.recent_repos.len()
        } else {
            self.results.len()
        }
    }
}

// ── Background task messages ──────────────────────────────────────────────────

pub(crate) enum BgMsg {
    PrsLoaded(Vec<PullRequest>),
    PrsError(String),
    ReviewDecisionsLoaded(PrMetadata),
    DiffLoaded(Vec<DiffFile>),
    DiffError(String),
    CommentsLoaded(Vec<ReviewComment>),
    CommentsError(String),
    DifftLoaded(Vec<(String, String)>),
    DifftError(String),
    CheckRunsLoaded(Vec<CheckRun>),
    CheckRunsError(String),
    ReviewSubmitted,
    ReviewError(String),
    InlineCommentSubmitted,
    InlineCommentError(String),
    /// A newer GitHub Release is available; contains the new tag string (e.g. `"v0.2.0"`).
    UpdateAvailable(String),
    /// The `cargo install` update finished successfully.
    UpdateCompleted,
    /// The `cargo install` update failed; contains the error message.
    UpdateFailed(String),
    /// Repo search results are ready.
    RepoSearchResults(Vec<RepoSearchResult>),
    /// Repo search failed.
    RepoSearchError(String),
    /// A new GitHubClient authenticated for a different owner is ready.
    GithubClientReady(Arc<GitHubClient>),
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub detail_tab: DetailTab,
    pub detail_focus: DetailFocus,
    pub show_help: bool,
    pub show_file_tree: bool,
    pub file_tree_cursor: usize,
    pub show_theme_picker: bool,
    pub theme_picker_cursor: usize,
    pub theme_picker_original: Option<Arc<Theme>>,
    pub config: Config,
    pub theme: Arc<Theme>,
    pub syntax_hl: Arc<SyntaxHighlighter>,

    pub repo: Option<RepoInfo>,
    pub github: Arc<GitHubClient>,

    pub prs: Vec<PullRequest>,
    pub pr_cursor: usize,
    pub pr_load_state: LoadState,

    pub diff_files: Vec<DiffFile>,
    pub diff_scroll: u16,
    pub diff_hscroll: u16,
    pub diff_file_cursor: usize,
    /// Index of the highlighted diff row (hunk headers + diff lines, 0-based).
    pub diff_line_cursor: usize,
    pub diff_load_state: LoadState,
    pub last_diff_area_height: u16,

    pub pr_comments: Vec<ReviewComment>,
    pub comments_scroll: u16,
    pub comments_load_state: LoadState,

    pub difft_files: Vec<(String, String)>, // (filename, raw ansi output)
    pub difft_scroll: u16,
    pub difft_hscroll: u16,
    pub difft_file_cursor: usize,
    pub difft_load_state: LoadState,

    // Vim motion state
    pub pending_count: String,
    pub g_pending: bool,

    pub check_runs: Vec<CheckRun>,
    pub check_runs_load_state: LoadState,

    /// Active review-input overlay (None when not shown).
    pub review_overlay: Option<ReviewOverlayState>,
    /// Read-only comment peek overlay: shows existing comments on the cursor diff line.
    pub comment_peek: Option<Vec<ReviewComment>>,
    /// Transient status message shown in the status bar (e.g. "Review submitted").
    /// Carries the time it was set so it can auto-expire after a few seconds.
    pub status_message: Option<(String, Instant)>,
    /// Some(tag) when a newer GitHub Release has been detected; drives the update prompt overlay.
    pub update_available: Option<String>,
    /// True while `cargo install` is running in the background during an update.
    pub update_in_progress: bool,
    /// Active repo-switcher overlay state (None when not shown).
    pub repo_switcher: Option<RepoSwitcherState>,

    pub(crate) tx: mpsc::UnboundedSender<BgMsg>,
    rx: mpsc::UnboundedReceiver<BgMsg>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = config::load()?;

        // Detect CWD repo first so we can pick the right gh account
        let repo = git::detect_repo(&std::env::current_dir()?)
            .ok()
            .or_else(|| {
                config.ui.last_repo.as_ref().and_then(|s| {
                    let mut parts = s.splitn(2, '/');
                    Some(RepoInfo {
                        owner: parts.next()?.to_string(),
                        name: parts.next()?.to_string(),
                        workdir: None,
                    })
                })
            });

        let owner_hint = repo.as_ref().map(|r| r.owner.as_str()).unwrap_or("");
        let github = Arc::new(GitHubClient::new_for_owner(owner_hint).await?);

        let theme = Arc::new(Theme::from_name(&config.ui.theme));

        let (tx, rx) = mpsc::unbounded_channel();

        let app = Self {
            screen: Screen::PrList,
            detail_tab: DetailTab::Diff,
            detail_focus: DetailFocus::Content,
            show_help: false,
            show_file_tree: false,
            file_tree_cursor: 0,
            show_theme_picker: false,
            theme_picker_cursor: Theme::index_of(&config.ui.theme),
            theme_picker_original: None,
            theme,
            syntax_hl: Arc::new(SyntaxHighlighter::new()),
            config,
            repo,
            github,
            prs: Vec::new(),
            pr_cursor: 0,
            pr_load_state: LoadState::Loading,
            diff_files: Vec::new(),
            diff_scroll: 0,
            diff_hscroll: 0,
            diff_file_cursor: 0,
            diff_line_cursor: 0,
            diff_load_state: LoadState::Idle,
            last_diff_area_height: DEFAULT_DIFF_AREA_HEIGHT,
            pr_comments: Vec::new(),
            comments_scroll: 0,
            comments_load_state: LoadState::Idle,
            difft_files: Vec::new(),
            difft_scroll: 0,
            difft_hscroll: 0,
            difft_file_cursor: 0,
            difft_load_state: LoadState::Idle,
            pending_count: String::new(),
            g_pending: false,
            check_runs: Vec::new(),
            check_runs_load_state: LoadState::Idle,
            review_overlay: None,
            comment_peek: None,
            status_message: None,
            update_available: None,
            update_in_progress: false,
            repo_switcher: None,
            tx,
            rx,
        };

        // Kick off initial PR load
        app.fetch_prs();
        // Kick off background update check (silent on failure)
        app.check_for_update();
        Ok(app)
    }

    // ── Main event loop ───────────────────────────────────────────────────────

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            // Drain background messages
            while let Ok(msg) = self.rx.try_recv() {
                self.handle_bg_msg(msg);
            }

            // Debounce repo search: fire search 300 ms after the last keystroke.
            self.tick_repo_search_debounce();

            // Expire stale status messages after STATUS_MESSAGE_TTL.
            if matches!(&self.status_message, Some((_, t)) if t.elapsed() > STATUS_MESSAGE_TTL) {
                self.status_message = None;
            }

            // Draw
            terminal.draw(|f| ui::render(f, self))?;

            // Poll for keyboard event (~60fps)
            if event::poll(TICK_RATE)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.handle_key(key.code, key.modifiers) {
                            break; // quit
                        }
                    }
                }
            }
        }

        // Persist config on exit
        self.save_config();
        Ok(())
    }

    // ── Key handling ──────────────────────────────────────────────────────────

    /// Returns `true` if the app should quit.
    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        // Update prompt intercepts y/n/Esc when active (but not while the
        // install is already running — no accidental double-trigger).
        if self.update_available.is_some() && !self.update_in_progress {
            return self.handle_key_update_prompt(code);
        }

        // Review input overlay intercepts everything when open.
        if self.review_overlay.is_some() {
            return self.handle_key_review_overlay(code, mods);
        }

        // Repo-switcher overlay intercepts everything when open.
        if self.repo_switcher.is_some() {
            return self.handle_key_repo_switcher(code, mods);
        }

        // Comment peek overlay: any key closes it.
        if self.comment_peek.is_some() {
            self.comment_peek = None;
            return false;
        }

        // Theme picker intercepts everything when open.
        if self.show_theme_picker {
            return self.handle_key_theme_picker(code);
        }

        // Help overlay intercepts everything except ?
        if self.show_help {
            self.show_help = false;
            return false;
        }

        match &self.screen {
            Screen::PrList => self.handle_key_list(code),
            Screen::PrDetail => self.handle_key_detail(code, mods),
        }
    }

    fn handle_key_review_overlay(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        crate::input::overlay::handle_key_review_overlay(self, code, mods)
    }

    /// Validate and submit the current review overlay.
    pub fn submit_review_overlay(&mut self) {
        let Some(overlay) = &mut self.review_overlay else { return };

        let body = overlay.body();
        if overlay.action.body_required() && body.trim().is_empty() {
            overlay.error = Some("A comment is required for this review type".to_string());
            return;
        }

        let Some(pr) = self.prs.get(self.pr_cursor) else { return };
        let pr_number = pr.number;
        let Some(repo) = self.repo.clone() else { return };
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();

        // Clone the action before moving out of the overlay.
        let action = overlay.action.clone();
        self.review_overlay = None;

        match action {
            ReviewAction::InlineComment { path, line, side } => {
                let commit_id = pr.head_sha.clone();
                tokio::spawn(async move {
                    match gh
                        .create_review_comment(
                            &repo.owner,
                            &repo.name,
                            pr_number,
                            &commit_id,
                            &path,
                            line,
                            &side,
                            &body,
                        )
                        .await
                    {
                        Ok(()) => { let _ = tx.send(BgMsg::InlineCommentSubmitted); }
                        Err(e) => { let _ = tx.send(BgMsg::InlineCommentError(format!("{:#}", e))); }
                    }
                });
            }
            _ => {
                let event = action.event_str().to_string();
                tokio::spawn(async move {
                    match gh.submit_review(&repo.owner, &repo.name, pr_number, &event, &body).await {
                        Ok(()) => { let _ = tx.send(BgMsg::ReviewSubmitted); }
                        Err(e) => { let _ = tx.send(BgMsg::ReviewError(format!("{:#}", e))); }
                    }
                });
            }
        }
    }

    /// Handle keypresses when the update-available prompt is visible.
    /// Returns `true` if the app should quit (it won't — we return false always
    /// here and let the app stay alive until the user relaunches after update).
    fn handle_key_update_prompt(&mut self, code: KeyCode) -> bool {
        crate::input::overlay::handle_key_update_prompt(self, code)
    }

    fn handle_key_theme_picker(&mut self, code: KeyCode) -> bool {
        crate::input::overlay::handle_key_theme_picker(self, code)
    }

    fn handle_key_repo_switcher(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        crate::input::overlay::handle_key_repo_switcher(self, code, mods)
    }

    fn handle_key_list(&mut self, code: KeyCode) -> bool {
        crate::input::list::handle_key_list(self, code)
    }

    fn handle_key_detail(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        crate::input::detail::handle_key_detail(self, code, mods)
    }

    // ── Background message handler ────────────────────────────────────────────

    fn handle_bg_msg(&mut self, msg: BgMsg) {
        match msg {
            BgMsg::PrsLoaded(prs) => {
                self.prs = prs;
                self.pr_load_state = LoadState::Idle;
                self.fetch_review_decisions();
            }
            BgMsg::PrsError(e) => {
                self.pr_load_state = LoadState::Error(e);
            }
            BgMsg::ReviewDecisionsLoaded(meta) => {
                for pr in &mut self.prs {
                    if let Some(decision) = meta.review_decisions.get(&pr.number) {
                        pr.review_decision = Some(decision.clone());
                    }
                    if let Some(ci_state) = meta.ci_statuses.get(&pr.number) {
                        pr.ci_status = Some(ci_state.clone());
                    }
                }
            }
            BgMsg::DiffLoaded(files) => {
                self.diff_files = files;
                self.diff_load_state = LoadState::Idle;
                self.diff_file_cursor = 0;
                self.diff_scroll = 0;
                self.diff_line_cursor = 0;
            }
            BgMsg::DiffError(e) => {
                self.diff_load_state = LoadState::Error(e);
            }
            BgMsg::CommentsLoaded(comments) => {
                self.pr_comments = comments;
                self.comments_load_state = LoadState::Idle;
                self.comments_scroll = 0;
            }
            BgMsg::CommentsError(e) => {
                self.comments_load_state = LoadState::Error(e);
            }
            BgMsg::DifftLoaded(files) => {
                self.difft_files = files;
                self.difft_load_state = LoadState::Idle;
                self.difft_file_cursor = 0;
                self.difft_scroll = 0;
            }
            BgMsg::DifftError(e) => {
                self.difft_load_state = LoadState::Error(e);
            }
            BgMsg::CheckRunsLoaded(runs) => {
                self.check_runs = runs;
                self.check_runs_load_state = LoadState::Idle;
            }
            BgMsg::CheckRunsError(e) => {
                self.check_runs_load_state = LoadState::Error(e);
            }
            BgMsg::ReviewSubmitted => {
                self.status_message = Some(("Review submitted successfully".to_string(), Instant::now()));
                // Refresh review decisions so the badge updates immediately
                self.fetch_review_decisions();
            }
            BgMsg::ReviewError(e) => {
                self.status_message = Some((format!("Review error: {e}"), Instant::now()));
            }
            BgMsg::InlineCommentSubmitted => {
                self.status_message = Some(("Inline comment posted".to_string(), Instant::now()));
                // Re-fetch comments so the new one appears in the Comments tab
                if let Some(pr) = self.prs.get(self.pr_cursor) {
                    let pr_number = pr.number;
                    self.fetch_comments(pr_number);
                }
            }
            BgMsg::InlineCommentError(e) => {
                self.status_message = Some((format!("Comment error: {e}"), Instant::now()));
            }
            BgMsg::UpdateAvailable(tag) => {
                self.update_available = Some(tag);
            }
            BgMsg::UpdateCompleted => {
                self.update_in_progress = false;
                self.update_available = None;
                self.status_message = Some((
                    "Update complete! Relaunch prr to use the new version.".to_string(),
                    Instant::now(),
                ));
            }
            BgMsg::UpdateFailed(e) => {
                self.update_in_progress = false;
                self.update_available = None;
                self.status_message = Some((format!("Update failed: {e}"), Instant::now()));
            }
            BgMsg::RepoSearchResults(results) => {
                if let Some(state) = self.repo_switcher.as_mut() {
                    state.results = results;
                    state.search_state = SearchState::Done;
                    state.cursor = 0;
                }
            }
            BgMsg::RepoSearchError(e) => {
                if let Some(state) = self.repo_switcher.as_mut() {
                    state.search_state = SearchState::Error(e);
                    state.results.clear();
                }
            }
            BgMsg::GithubClientReady(client) => {
                // Swap in the new authenticated client, then re-fetch PRs with it.
                self.github = client;
                self.prs = Vec::new();
                self.pr_load_state = LoadState::Loading;
                self.fetch_prs();
            }
        }
    }

    // ── Async task launchers ──────────────────────────────────────────────────

    /// Fire a repo search in the background if the debounce window has elapsed.
    fn tick_repo_search_debounce(&mut self) {
        let should_fire = self
            .repo_switcher
            .as_ref()
            .and_then(|s| s.last_keystroke)
            .map(|t| t.elapsed() >= SEARCH_DEBOUNCE)
            .unwrap_or(false);

        if should_fire {
            if let Some(state) = self.repo_switcher.as_mut() {
                // Clear the debounce timer so we don't re-fire on the next tick.
                state.last_keystroke = None;
            }
            let query = self
                .repo_switcher
                .as_ref()
                .map(|s| s.query.clone())
                .unwrap_or_default();
            if !query.is_empty() {
                self.search_repos_bg(query);
            }
        }
    }

    /// Spawn a background task to search GitHub repos for `query`.
    fn search_repos_bg(&self, query: String) {
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match gh.search_repos(&query, SEARCH_PAGE_SIZE).await {
                Ok(results) => {
                    let _ = tx.send(BgMsg::RepoSearchResults(results));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::RepoSearchError(format!("{:#}", e)));
                }
            }
        });
    }

    /// Switch the active repo to `full_name` ("owner/name"), re-authenticate if
    /// needed, persist to recent history, and trigger a fresh PR load.
    pub fn switch_repo(&mut self, full_name: &str) {
        let mut parts = full_name.splitn(2, '/');
        let owner = match parts.next() {
            Some(o) if !o.is_empty() => o.to_string(),
            _ => return,
        };
        let name = match parts.next() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => return,
        };

        // Detect whether the owner is changing — if so, we need a new token.
        let owner_changed = self
            .repo
            .as_ref()
            .map(|r| !r.owner.eq_ignore_ascii_case(&owner))
            .unwrap_or(true);

        // Update the active repo (no local workdir — it wasn't checked out here).
        self.repo = Some(RepoInfo { owner: owner.clone(), name: name.clone(), workdir: None });

        // Persist to recent repos history.
        self.config.ui.push_recent_repo(&owner, &name);

        // Reset the PR list state.
        self.prs = Vec::new();
        self.pr_cursor = 0;
        self.pr_load_state = LoadState::Loading;

        if owner_changed {
            // Kick off a background re-auth for the new owner.  When the new
            // client arrives via BgMsg::GithubClientReady it will re-fetch PRs
            // with the correct token.  We also do a speculative fetch with the
            // current token — if the new repo is public it will work immediately.
            self.reauth_for_owner(owner);
        }
        self.fetch_prs();
    }

    /// Spawn a background task that authenticates for `owner` and sends back a
    /// ready-to-use `GitHubClient` via `BgMsg::GithubClientReady`.
    fn reauth_for_owner(&self, owner: String) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if let Ok(client) = GitHubClient::new_for_owner(&owner).await {
                let _ = tx.send(BgMsg::GithubClientReady(Arc::new(client)));
            }
        });
    }

    /// Spawn a background task that checks the GitHub Releases API for a newer
    /// version.  The result arrives via `BgMsg::UpdateAvailable`.
    fn check_for_update(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if let Some(tag) = updater::check_for_update().await {
                let _ = tx.send(BgMsg::UpdateAvailable(tag));
            }
        });
    }

    pub fn fetch_prs(&self) {
        let Some(repo) = self.repo.clone() else { return };
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();
        let mine_only = !self.config.ui.show_all_prs;

        tokio::spawn(async move {
            match gh.list_prs(&repo.owner, &repo.name, mine_only).await {
                Ok(prs) => { let _ = tx.send(BgMsg::PrsLoaded(prs)); }
                Err(e) => { let _ = tx.send(BgMsg::PrsError(format!("{:#}", e))); }
            }
        });
    }

    fn fetch_review_decisions(&self) {
        let Some(repo) = self.repo.clone() else { return };
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();

        tokio::spawn(async move {
                match gh.fetch_review_decisions(&repo.owner, &repo.name).await {
                    Ok(meta) => { let _ = tx.send(BgMsg::ReviewDecisionsLoaded(meta)); }
                    Err(_) => {} // silently ignore — review decisions are best-effort
                }
        });
    }

    fn fetch_diff(&self, pr_number: u64) {
        let Some(repo) = self.repo.clone() else { return };
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match gh.pr_diff(&repo.owner, &repo.name, pr_number).await {
                Ok(raw) => {
                    let files = parse_diff(&raw);
                    let _ = tx.send(BgMsg::DiffLoaded(files));
                }
                Err(e) => { let _ = tx.send(BgMsg::DiffError(format!("{:#}", e))); }
            }
        });
    }

    fn fetch_comments(&self, pr_number: u64) {
        let Some(repo) = self.repo.clone() else { return };
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match gh.pr_comments(&repo.owner, &repo.name, pr_number).await {
                Ok(comments) => { let _ = tx.send(BgMsg::CommentsLoaded(comments)); }
                Err(e) => { let _ = tx.send(BgMsg::CommentsError(format!("{:#}", e))); }
            }
        });
    }

    fn fetch_check_runs(&self, head_sha: String) {
        let Some(repo) = self.repo.clone() else { return };
        let gh = Arc::clone(&self.github);
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match gh.fetch_check_runs(&repo.owner, &repo.name, &head_sha).await {
                Ok(runs) => { let _ = tx.send(BgMsg::CheckRunsLoaded(runs)); }
                Err(e) => { let _ = tx.send(BgMsg::CheckRunsError(format!("{:#}", e))); }
            }
        });
    }

    fn fetch_difft(&self, base_sha: String, head_sha: String) {
        let Some(repo) = self.repo.clone() else { return };
        let tx = self.tx.clone();

        // Require a local workdir — the branch must be checked out.
        let Some(workdir) = repo.workdir.clone() else {
            let _ = tx.send(BgMsg::DifftError(
                "Branch not checked out locally — press 'c' to checkout".to_string(),
            ));
            return;
        };

        tokio::task::spawn_blocking(move || {
            // Verify difft is available.
            if std::process::Command::new("difft")
                .arg("--version")
                .output()
                .is_err()
            {
                let _ = tx.send(BgMsg::DifftError(
                    "difft not found — install difftastic (https://difftastic.wilfred.me.uk/installation.html)".to_string(),
                ));
                return;
            }

            // Run git diff with difftastic as the external diff tool.
            // git invokes `difft DISPLAY_PATH OLD NEW OLD_HEX OLD_MODE NEW_HEX NEW_MODE`
            // once per changed file, with correct display paths — zero temp file management.
            let output = std::process::Command::new("git")
                .args([
                    "-c",
                    "diff.external=difft --color=always --skip-unchanged",
                    "diff",
                    "--ext-diff",
                    &format!("{}...{}", base_sha, head_sha),
                ])
                .current_dir(&workdir)
                .output();

            let stdout = match output {
                Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
                Err(e) => {
                    let _ = tx.send(BgMsg::DifftError(format!("git diff failed: {}", e)));
                    return;
                }
            };

            if stdout.trim().is_empty() {
                let _ = tx.send(BgMsg::DifftLoaded(Vec::new()));
                return;
            }

            // Split combined output into per-file sections.
            // Each file's output starts with a header line: "<filename> --- <Language>"
            // (with ANSI codes around the filename). We detect these by stripping ANSI
            // and checking for the " --- " separator pattern.
            let results = split_difft_output(&stdout);
            let _ = tx.send(BgMsg::DifftLoaded(results));
        });
    }

    // ── Scroll helpers ────────────────────────────────────────────────────────

    /// Total rendered lines for the current diff file (each hunk header + each diff line).
    pub fn max_diff_scroll(&self) -> u16 {
        let idx = self.diff_file_cursor.min(self.diff_files.len().saturating_sub(1));
        let total: usize = self.diff_files.get(idx).map(|f| {
            f.hunks.iter().map(|h| 1 + h.lines.len()).sum()
        }).unwrap_or(0);
        (total as u16).saturating_sub(1)
    }

    /// Resolve the diff line at `diff_line_cursor` in the current file.
    ///
    /// The rendered rows interleave hunk-header rows (no line number) with diff
    /// lines.  This method walks the hunks in order, counting each hunk header
    /// as one row, and returns the `DiffLine` when the cursor lands on an actual
    /// diff line (skipping hunk-header rows).
    ///
    /// Returns `(file_path, diff_line)` if the cursor is on a commentable row,
    /// or `None` when it is on a hunk-header row or out of bounds.
    pub fn diff_line_at_cursor(&self) -> Option<(String, &crate::github::DiffLine)> {
        let file = self.diff_files.get(self.diff_file_cursor)?;
        let mut row = 0usize;
        for hunk in &file.hunks {
            // hunk-header row
            if row == self.diff_line_cursor {
                return None; // cursor is on a hunk header — not commentable
            }
            row += 1;
            for diff_line in &hunk.lines {
                if row == self.diff_line_cursor {
                    return Some((file.filename.clone(), diff_line));
                }
                row += 1;
            }
        }
        None
    }

    /// Return any existing review comments attached to the current cursor diff line.
    /// Returns `None` if the cursor is on a hunk header or there are no comments.
    pub fn comments_at_cursor(&self) -> Option<Vec<ReviewComment>> {
        let (path, diff_line) = self.diff_line_at_cursor()?;
        // Use right_no for Added/Context lines, left_no for Removed — matching the
        // marker logic in the diff renderer.
        let line_no: u64 = match diff_line.kind {
            crate::github::DiffLineKind::Removed => diff_line.left_no? as u64,
            _ => diff_line.right_no? as u64,
        };
        let matches: Vec<ReviewComment> = self
            .pr_comments
            .iter()
            .filter(|c| {
                c.path.as_deref() == Some(&path)
                    && c.line.map(|l| l == line_no).unwrap_or(false)
            })
            .cloned()
            .collect();
        if matches.is_empty() { None } else { Some(matches) }
    }

    /// Total rendered lines for comments.
    pub fn max_comments_scroll(&self) -> u16 {
        // Each comment: 1 header line + body lines + 1 separator
        let total: usize = self.pr_comments.iter().map(|c| {
            1 + c.body.lines().count() + 1
        }).sum();
        (total as u16).saturating_sub(1)
    }

    /// Total rendered lines for the current difftastic file output.
    pub fn max_difft_scroll(&self) -> u16 {
        let idx = self.difft_file_cursor.min(self.difft_files.len().saturating_sub(1));
        let total = self.difft_files.get(idx)
            .map(|(_, raw)| raw.lines().count())
            .unwrap_or(0);
        (total as u16).saturating_sub(1)
    }

    // ── Actions ───────────────────────────────────────────────────────────────

    pub fn open_detail(&mut self) {
        let Some(pr) = self.prs.get(self.pr_cursor) else { return };
        let pr_number = pr.number;
        let head_sha = pr.head_sha.clone();
        let base_sha = pr.base_sha.clone();

        self.screen = Screen::PrDetail;
        self.detail_tab = DetailTab::Diff;
        self.diff_files = Vec::new();
        self.diff_load_state = LoadState::Loading;
        self.pr_comments = Vec::new();
        self.comments_load_state = LoadState::Loading;
        self.difft_files = Vec::new();
        self.difft_load_state = LoadState::Loading;
        self.check_runs = Vec::new();
        self.check_runs_load_state = LoadState::Loading;

        self.fetch_diff(pr_number);
        self.fetch_comments(pr_number);
        self.fetch_difft(base_sha, head_sha.clone());
        self.fetch_check_runs(head_sha);
    }

    /// Returns the set of file indices (into `diff_files`) that are marked reviewed
    /// for the currently open PR.
    pub fn reviewed_diff_indices(&self) -> HashSet<usize> {
        let Some(repo) = &self.repo else { return HashSet::new() };
        let Some(pr) = self.prs.get(self.pr_cursor) else { return HashSet::new() };
        self.diff_files
            .iter()
            .enumerate()
            .filter(|(_, f)| self.config.is_reviewed(&repo.owner, &repo.name, pr.number, &f.filename))
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns the set of file indices (into `difft_files`) that are marked reviewed
    /// for the currently open PR.
    pub fn reviewed_difft_indices(&self) -> HashSet<usize> {
        let Some(repo) = &self.repo else { return HashSet::new() };
        let Some(pr) = self.prs.get(self.pr_cursor) else { return HashSet::new() };
        self.difft_files
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| self.config.is_reviewed(&repo.owner, &repo.name, pr.number, name))
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns (reviewed_count, total_count) for the current PR's diff files.
    pub fn reviewed_progress(&self) -> (usize, usize) {
        let total = self.diff_files.len();
        let reviewed = self.reviewed_diff_indices().len();
        (reviewed, total)
    }

    /// Toggle the current file's reviewed state and auto-advance to the next unreviewed file.
    pub fn toggle_reviewed(&mut self) {
        let Some(repo) = &self.repo else { return };
        let Some(pr) = self.prs.get(self.pr_cursor) else { return };
        let pr_number = pr.number;
        let owner = repo.owner.clone();
        let repo_name = repo.name.clone();

        // Resolve current file cursor, total file count, and filename in one match.
        let (file_cursor, total_files, filename) = match self.detail_tab {
            DetailTab::Diff => (
                self.diff_file_cursor,
                self.diff_files.len(),
                self.diff_files.get(self.diff_file_cursor).map(|f| f.filename.clone()),
            ),
            DetailTab::Difftastic => (
                self.difft_file_cursor,
                self.difft_files.len(),
                self.difft_files.get(self.difft_file_cursor).map(|(n, _)| n.clone()),
            ),
            DetailTab::Comments => return,
        };
        let Some(filename) = filename else { return };

        self.config.toggle_reviewed(&owner, &repo_name, pr_number, &filename);

        // Auto-advance to next unreviewed file (wrap around)
        if total_files > 1 {
            let next = (1..total_files)
                .map(|offset| (file_cursor + offset) % total_files)
                .find(|&idx| {
                    let fname = match self.detail_tab {
                        DetailTab::Diff => self.diff_files.get(idx).map(|f| f.filename.as_str()),
                        DetailTab::Difftastic => {
                            self.difft_files.get(idx).map(|(n, _)| n.as_str())
                        }
                        DetailTab::Comments => None,
                    };
                    fname.map_or(false, |f| {
                        !self.config.is_reviewed(&owner, &repo_name, pr_number, f)
                    })
                });

            if let Some(next_idx) = next {
                match self.detail_tab {
                    DetailTab::Diff => {
                        self.diff_file_cursor = next_idx;
                        self.diff_scroll = 0;
                    }
                    DetailTab::Difftastic => {
                        self.difft_file_cursor = next_idx;
                        self.difft_scroll = 0;
                    }
                    DetailTab::Comments => {}
                }
            }
        }
    }

    pub fn checkout_pr_branch(&mut self) {
        let Some(pr) = self.prs.get(self.pr_cursor) else { return };
        let branch = pr.head_branch.clone();

        // Run git checkout in a blocking thread so we don't block the async runtime
        tokio::task::spawn_blocking(move || {
            let _ = Command::new("git")
                .args(["checkout", &branch])
                .status();
        });
    }

    fn save_config(&mut self) {
        if let Some(repo) = &self.repo {
            self.config.ui.last_repo = Some(repo.full_name());
            self.config.ui.push_recent_repo(&repo.owner.clone(), &repo.name.clone());
        }
        let _ = config::save(&self.config);
    }
}

// ── Repo switcher helpers ─────────────────────────────────────────────────────

/// Returns `true` when `s` looks like a complete "owner/repo" slug:
/// exactly one `/`, non-empty on both sides, no further slashes.
pub fn looks_like_full_name(s: &str) -> bool {
    let mut parts = s.splitn(3, '/');
    let owner = parts.next().unwrap_or("");
    let repo = parts.next().unwrap_or("");
    let extra = parts.next();
    !owner.is_empty() && !repo.is_empty() && extra.is_none()
}

// ── Difftastic output parser ──────────────────────────────────────────────────

/// Split the combined `git diff --ext-diff` output into per-file sections.
///
/// Difftastic emits one header line **per hunk**, not per file:
///   - Single-hunk file:  `filename --- Language`
///   - Multi-hunk file:   `filename --- 1/3 --- Language`
///                        `filename --- 2/3 --- Language`
///                        ...
///
/// We detect hunk headers, group all hunks for the same filename into one
/// entry, and include every hunk header line so the TUI shows them in context.
fn split_difft_output(stdout: &str) -> Vec<(String, String)> {
    // Collect (filename, hunk_index, raw_line_slice) tuples first.
    // hunk_index = 1 for "first hunk of a file" (or 0 when no counter).
    let mut sections: Vec<(String, usize, Vec<&str>)> = Vec::new();
    let mut cur_lines: Vec<&str> = Vec::new();
    let mut cur_name = String::new();
    let mut cur_hunk: usize = 0;

    for line in stdout.lines() {
        let stripped = strip_ansi(line);
        if let Some((filename, hunk_idx)) = detect_hunk_header(&stripped) {
            if !cur_name.is_empty() {
                sections.push((cur_name.clone(), cur_hunk, cur_lines.clone()));
                cur_lines.clear();
            }
            cur_name = filename;
            cur_hunk = hunk_idx;
            cur_lines.push(line);
        } else {
            cur_lines.push(line);
        }
    }
    if !cur_name.is_empty() {
        sections.push((cur_name, cur_hunk, cur_lines));
    }

    // Merge hunks that belong to the same file into a single entry.
    // A new file starts when hunk_index == 1 (or 0 for single-hunk files).
    // Store the raw ANSI string so the render path can apply treesitter highlighting.
    let mut results: Vec<(String, String)> = Vec::new();

    for (filename, hunk_idx, raw_lines) in sections {
        let is_new_file = hunk_idx <= 1;

        if is_new_file {
            // Start a fresh entry — store raw ANSI text.
            results.push((filename, raw_lines.join("\n")));
        } else {
            // Append to the last entry (same file, subsequent hunk).
            if let Some(last) = results.last_mut() {
                last.1.push('\n');
                last.1.push_str(&raw_lines.join("\n"));
            }
        }
    }

    results
}

/// Detect a difftastic hunk header line (ANSI already stripped).
///
/// Formats:
///   `filename --- Language`            → returns (filename, 0)  [single hunk]
///   `filename --- N/M --- Language`    → returns (filename, N)  [hunk N of M]
///
/// Guards against false positives from diff content lines (which start with
/// digits for line-number columns).
fn detect_hunk_header(stripped: &str) -> Option<(String, usize)> {
    let sep = " --- ";

    // The filename is everything before the first " --- ".
    let first_sep = stripped.find(sep)?;
    let filename = stripped[..first_sep].trim();

    // Filename must be non-empty and must not start with a digit or space.
    // (Diff content lines always start with spaces+digits for line-number columns.)
    if filename.is_empty() || filename.starts_with(|c: char| c.is_ascii_digit() || c == ' ') {
        return None;
    }

    let after = &stripped[first_sep + sep.len()..];

    // Check if after is "N/M --- Language" (multi-hunk) or just "Language".
    // We look for a second " --- " in `after`.
    if let Some(second_sep) = after.find(sep) {
        // Multi-hunk: after[..second_sep] should be "N/M"
        let hunk_part = &after[..second_sep];
        if let Some(slash) = hunk_part.find('/') {
            let n: usize = hunk_part[..slash].trim().parse().ok()?;
            return Some((filename.to_string(), n));
        }
        // Unexpected format — treat as new file.
        Some((filename.to_string(), 1))
    } else {
        // Single-hunk file: after is just "Language" (or "Language (note)").
        // Language must not be empty and must not start with a digit.
        let lang = after.trim();
        if lang.is_empty() || lang.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        Some((filename.to_string(), 0))
    }
}

/// Strip ANSI escape sequences from a string for pattern matching.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            i += 2;
            while i < len && !bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            i += 1; // skip the terminating letter
        } else {
            let ch_len = utf8_char_len(bytes[i]);
            if i + ch_len <= len {
                out.push_str(&s[i..i + ch_len]);
            }
            i += ch_len;
        }
    }
    out
}

/// Return the byte length of a UTF-8 encoded character given its leading byte.
#[inline]
fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 { 1 }
    else if b < 0xE0 { 2 }
    else if b < 0xF0 { 3 }
    else { 4 }
}

#[cfg(test)]
mod difft_parser_tests {
    use super::{detect_hunk_header, split_difft_output};

    // ── detect_hunk_header ────────────────────────────────────────────────────

    #[test]
    fn single_hunk_rust() {
        let h = detect_hunk_header("src/config.rs --- Rust").unwrap();
        assert_eq!(h.0, "src/config.rs");
        assert_eq!(h.1, 0);
    }

    #[test]
    fn single_hunk_text() {
        let h = detect_hunk_header(".gitignore --- Text").unwrap();
        assert_eq!(h.0, ".gitignore");
        assert_eq!(h.1, 0);
    }

    #[test]
    fn multi_hunk_first() {
        let h = detect_hunk_header("src/git.rs --- 1/3 --- Rust").unwrap();
        assert_eq!(h.0, "src/git.rs");
        assert_eq!(h.1, 1);
    }

    #[test]
    fn multi_hunk_middle() {
        let h = detect_hunk_header("src/git.rs --- 2/3 --- Rust").unwrap();
        assert_eq!(h.0, "src/git.rs");
        assert_eq!(h.1, 2);
    }

    #[test]
    fn exceeded_graph_limit() {
        // difft falls back to Text with a note
        let h = detect_hunk_header("src/app.rs --- 1/13 --- Text (exceeded DFT_GRAPH_LIMIT)").unwrap();
        assert_eq!(h.0, "src/app.rs");
        assert_eq!(h.1, 1);
    }

    #[test]
    fn content_line_space_prefix_rejected() {
        // Typical diff content line with left+right line numbers
        assert!(detect_hunk_header(" 1  1 use anyhow;").is_none());
        assert!(detect_hunk_header("  2  2 use git2::Repository;").is_none());
    }

    #[test]
    fn content_line_digit_prefix_rejected() {
        assert!(detect_hunk_header("1 2 some code --- more code").is_none());
    }

    #[test]
    fn ellipsis_context_line_rejected() {
        // difft emits "..." between non-adjacent context hunks
        assert!(detect_hunk_header("...").is_none());
    }

    #[test]
    fn empty_line_rejected() {
        assert!(detect_hunk_header("").is_none());
    }

    // ── split_difft_output ────────────────────────────────────────────────────

    #[test]
    fn groups_multi_hunk_file() {
        let input = "\
src/git.rs --- 1/2 --- Rust\n\
 1  1 line one\n\
src/git.rs --- 2/2 --- Rust\n\
 2  2 line two\n";
        let result = split_difft_output(input);
        assert_eq!(result.len(), 1, "both hunks should be in one entry");
        assert_eq!(result[0].0, "src/git.rs");
        // 4 raw lines (two header lines + two content lines)
        assert_eq!(result[0].1.lines().count(), 4);
    }

    #[test]
    fn separate_files_become_separate_entries() {
        let input = "\
.gitignore --- Text\n\
 1  1 /target\n\
src/main.rs --- Rust\n\
 1  1 fn main() {}\n";
        let result = split_difft_output(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, ".gitignore");
        assert_eq!(result[1].0, "src/main.rs");
    }

    #[test]
    fn empty_input_gives_empty_result() {
        assert!(split_difft_output("").is_empty());
    }
}
