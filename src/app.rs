use crate::config::{self, Config};
use crate::git::{self, RepoInfo};
use crate::github::{CheckRun, DiffFile, GitHubClient, PrMetadata, PullRequest, ReviewComment, parse_diff};
use crate::syntax::SyntaxHighlighter;
use crate::ui;
use crate::ui::theme::{Theme, ALL_THEMES};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    collections::HashSet,
    io,
    process::Command,
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;

// ── State enums ───────────────────────────────────────────────────────────────

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

// ── Background task messages ──────────────────────────────────────────────────

enum BgMsg {
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
    pub theme_picker_original: Option<Theme>,
    pub config: Config,
    pub theme: Theme,
    pub syntax_hl: SyntaxHighlighter,

    pub repo: Option<RepoInfo>,
    pub github: Arc<GitHubClient>,

    pub prs: Vec<PullRequest>,
    pub pr_cursor: usize,
    pub pr_load_state: LoadState,

    pub diff_files: Vec<DiffFile>,
    pub diff_scroll: u16,
    pub diff_hscroll: u16,
    pub diff_file_cursor: usize,
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

    tx: mpsc::UnboundedSender<BgMsg>,
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

        let theme = Theme::from_name(&config.ui.theme);

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
            syntax_hl: SyntaxHighlighter::new(),
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
            diff_load_state: LoadState::Idle,
            last_diff_area_height: 24,
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
            tx,
            rx,
        };

        // Kick off initial PR load
        app.fetch_prs();
        Ok(app)
    }

    // ── Main event loop ───────────────────────────────────────────────────────

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            // Drain background messages
            while let Ok(msg) = self.rx.try_recv() {
                self.handle_bg_msg(msg);
            }

            // Draw
            terminal.draw(|f| ui::render(f, self))?;

            // Poll for keyboard event (16ms → ~60fps)
            if event::poll(Duration::from_millis(16))? {
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

    fn handle_key_theme_picker(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.theme_picker_cursor + 1 < ALL_THEMES.len() {
                    self.theme_picker_cursor += 1;
                    self.theme = Theme::from_name(ALL_THEMES[self.theme_picker_cursor].0);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.theme_picker_cursor > 0 {
                    self.theme_picker_cursor -= 1;
                    self.theme = Theme::from_name(ALL_THEMES[self.theme_picker_cursor].0);
                }
            }
            KeyCode::Enter => {
                self.config.ui.theme = ALL_THEMES[self.theme_picker_cursor].0.to_string();
                self.theme_picker_original = None;
                self.show_theme_picker = false;
            }
            KeyCode::Esc => {
                if let Some(original) = self.theme_picker_original.take() {
                    self.theme = original;
                }
                self.show_theme_picker = false;
            }
            _ => {}
        }
        false
    }

    fn handle_key_list(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.prs.is_empty() {
                    self.pr_cursor = (self.pr_cursor + 1).min(self.prs.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.pr_cursor = self.pr_cursor.saturating_sub(1);
            }
            KeyCode::Enter => {
                if !self.prs.is_empty() {
                    self.open_detail();
                }
            }
            KeyCode::Char('a') => {
                self.config.ui.show_all_prs = !self.config.ui.show_all_prs;
                self.pr_cursor = 0;
                self.pr_load_state = LoadState::Loading;
                self.fetch_prs();
            }
            KeyCode::Char('r') => {
                self.pr_cursor = 0;
                self.pr_load_state = LoadState::Loading;
                self.fetch_prs();
            }
            KeyCode::Char('o') => {
                if let Some(pr) = self.prs.get(self.pr_cursor) {
                    let _ = open::that(&pr.url);
                }
            }
            KeyCode::Char('T') => {
                self.theme_picker_original = Some(self.theme.clone());
                self.theme_picker_cursor = Theme::index_of(&self.config.ui.theme);
                self.show_theme_picker = true;
            }
            _ => {}
        }
        false
    }

    fn handle_key_detail(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        // ── File-tree focus: intercept j/k/Enter/Esc ─────────────────────────
        if self.detail_focus == DetailFocus::FileTree {
            self.pending_count.clear();
            self.g_pending = false;
            return self.handle_key_file_tree(code);
        }

        // ── Count prefix accumulation (digits 0-9) ───────────────────────────
        // '0' with an existing count prefix is a count digit; bare '0' is "scroll to column 0".
        if let KeyCode::Char(c) = code {
            if c.is_ascii_digit() && (c != '0' || !self.pending_count.is_empty()) {
                self.pending_count.push(c);
                self.g_pending = false;
                return false;
            }
        }

        // Consume the count (default 1) and clear the buffer.
        let count: u16 = self.pending_count.parse().unwrap_or(1).max(1);
        self.pending_count.clear();

        // ── Ctrl-d / Ctrl-u (half-page) ──────────────────────────────────────
        if mods.contains(KeyModifiers::CONTROL) {
            let half = (self.last_diff_area_height / 2).max(1);
            let step = half.saturating_mul(count);
            match code {
                KeyCode::Char('d') => {
                    self.g_pending = false;
                    match self.detail_tab {
                        DetailTab::Diff => {
                            let max = self.max_diff_scroll();
                            self.diff_scroll = self.diff_scroll.saturating_add(step).min(max);
                        }
                        DetailTab::Comments => {
                            let max = self.max_comments_scroll();
                            self.comments_scroll = self.comments_scroll.saturating_add(step).min(max);
                        }
                        DetailTab::Difftastic => {
                            let max = self.max_difft_scroll();
                            self.difft_scroll = self.difft_scroll.saturating_add(step).min(max);
                        }
                    }
                    return false;
                }
                KeyCode::Char('u') => {
                    self.g_pending = false;
                    match self.detail_tab {
                        DetailTab::Diff => self.diff_scroll = self.diff_scroll.saturating_sub(step),
                        DetailTab::Comments => self.comments_scroll = self.comments_scroll.saturating_sub(step),
                        DetailTab::Difftastic => self.difft_scroll = self.difft_scroll.saturating_sub(step),
                    }
                    return false;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.g_pending = false;
                self.screen = Screen::PrList;
                self.diff_scroll = 0;
                self.diff_hscroll = 0;
                self.comments_scroll = 0;
                self.diff_file_cursor = 0;
            }
            KeyCode::Char('?') => {
                self.g_pending = false;
                self.show_help = true;
            }
            // Space: toggle tree panel visibility
            KeyCode::Char(' ') => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff | DetailTab::Difftastic => {
                        self.show_file_tree = !self.show_file_tree;
                        if !self.show_file_tree {
                            self.detail_focus = DetailFocus::Content;
                        }
                    }
                    DetailTab::Comments => {} // tree not shown on Comments tab
                }
            }
            KeyCode::Tab => {
                self.g_pending = false;
                if self.show_file_tree
                    && matches!(self.detail_tab, DetailTab::Diff | DetailTab::Difftastic)
                {
                    // Cycle focus: Content → FileTree → Content
                    self.detail_focus = match self.detail_focus {
                        DetailFocus::Content => DetailFocus::FileTree,
                        DetailFocus::FileTree => DetailFocus::Content,
                    };
                } else {
                    // No tree visible: cycle tabs as before
                    self.detail_tab = match self.detail_tab {
                        DetailTab::Diff => DetailTab::Comments,
                        DetailTab::Comments => DetailTab::Difftastic,
                        DetailTab::Difftastic => DetailTab::Diff,
                    };
                }
            }

            // ── Vertical scroll: j / Down ────────────────────────────────────
            KeyCode::Char('j') | KeyCode::Down => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => {
                        let max = self.max_diff_scroll();
                        self.diff_scroll = self.diff_scroll.saturating_add(count).min(max);
                    }
                    DetailTab::Comments => {
                        let max = self.max_comments_scroll();
                        self.comments_scroll = self.comments_scroll.saturating_add(count).min(max);
                    }
                    DetailTab::Difftastic => {
                        let max = self.max_difft_scroll();
                        self.difft_scroll = self.difft_scroll.saturating_add(count).min(max);
                    }
                }
            }

            // ── Vertical scroll: k / Up ──────────────────────────────────────
            KeyCode::Char('k') | KeyCode::Up => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => self.diff_scroll = self.diff_scroll.saturating_sub(count),
                    DetailTab::Comments => self.comments_scroll = self.comments_scroll.saturating_sub(count),
                    DetailTab::Difftastic => self.difft_scroll = self.difft_scroll.saturating_sub(count),
                }
            }

            // ── Horizontal scroll: h / Left / l / Right ──────────────────────
            KeyCode::Char('h') | KeyCode::Left => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => self.diff_hscroll = self.diff_hscroll.saturating_sub(count),
                    DetailTab::Difftastic => self.difft_hscroll = self.difft_hscroll.saturating_sub(count),
                    DetailTab::Comments => {}
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => self.diff_hscroll = self.diff_hscroll.saturating_add(count),
                    DetailTab::Difftastic => self.difft_hscroll = self.difft_hscroll.saturating_add(count),
                    DetailTab::Comments => {}
                }
            }

            // ── G — jump to bottom ───────────────────────────────────────────
            KeyCode::Char('G') => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => self.diff_scroll = self.max_diff_scroll(),
                    DetailTab::Comments => self.comments_scroll = self.max_comments_scroll(),
                    DetailTab::Difftastic => self.difft_scroll = self.max_difft_scroll(),
                }
            }

            // ── g — first press arms gg; second press jumps to top ───────────
            KeyCode::Char('g') => {
                if self.g_pending {
                    // gg: jump to top
                    self.g_pending = false;
                    match self.detail_tab {
                        DetailTab::Diff => self.diff_scroll = 0,
                        DetailTab::Comments => self.comments_scroll = 0,
                        DetailTab::Difftastic => self.difft_scroll = 0,
                    }
                } else {
                    self.g_pending = true;
                }
            }

            // ── 0 — scroll to leftmost column (bare zero, no count prefix) ───
            KeyCode::Char('0') => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => self.diff_hscroll = 0,
                    DetailTab::Difftastic => self.difft_hscroll = 0,
                    DetailTab::Comments => {}
                }
            }

            // ── $ — scroll to far right (large sentinel value) ───────────────
            KeyCode::Char('$') => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Diff => self.diff_hscroll = u16::MAX,
                    DetailTab::Difftastic => self.difft_hscroll = u16::MAX,
                    DetailTab::Comments => {}
                }
            }

            // ── File navigation: n / N ────────────────────────────────────────
            KeyCode::Char('n') => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Difftastic => {
                        if !self.difft_files.is_empty() {
                            self.difft_file_cursor =
                                (self.difft_file_cursor + count as usize).min(self.difft_files.len() - 1);
                            self.difft_scroll = 0;
                        }
                    }
                    _ => {
                        if !self.diff_files.is_empty() {
                            self.diff_file_cursor =
                                (self.diff_file_cursor + count as usize).min(self.diff_files.len() - 1);
                            self.diff_scroll = 0;
                        }
                    }
                }
            }
            KeyCode::Char('N') => {
                self.g_pending = false;
                match self.detail_tab {
                    DetailTab::Difftastic => {
                        self.difft_file_cursor = self.difft_file_cursor.saturating_sub(count as usize);
                        self.difft_scroll = 0;
                    }
                    _ => {
                        self.diff_file_cursor = self.diff_file_cursor.saturating_sub(count as usize);
                        self.diff_scroll = 0;
                    }
                }
            }

            KeyCode::Char('o') => {
                self.g_pending = false;
                if let Some(pr) = self.prs.get(self.pr_cursor) {
                    let _ = open::that(&pr.url);
                }
            }
            KeyCode::Char('v') => {
                self.g_pending = false;
                self.toggle_reviewed();
            }
            KeyCode::Char('c') => {
                self.g_pending = false;
                self.checkout_pr_branch();
            }
            KeyCode::Char('T') => {
                self.g_pending = false;
                self.theme_picker_original = Some(self.theme.clone());
                self.theme_picker_cursor = Theme::index_of(&self.config.ui.theme);
                self.show_theme_picker = true;
            }
            _ => {
                // Any unrecognised key clears the g-pending state.
                self.g_pending = false;
            }
        }
        false
    }

    /// Key handler when the file-tree sidebar has focus.
    fn handle_key_file_tree(&mut self, code: KeyCode) -> bool {
        use crate::ui::file_tree;

        // Build current rows so we can do index arithmetic.
        let rows: Vec<file_tree::TreeRow> = match self.detail_tab {
            DetailTab::Diff => {
                let paths: Vec<String> =
                    self.diff_files.iter().map(|f| f.filename.clone()).collect();
                file_tree::build_rows(&paths)
            }
            DetailTab::Difftastic => {
                let paths: Vec<String> =
                    self.difft_files.iter().map(|(n, _)| n.clone()).collect();
                file_tree::build_rows(&paths)
            }
            DetailTab::Comments => {
                // Tree not shown on Comments; transfer focus back
                self.detail_focus = DetailFocus::Content;
                return false;
            }
        };

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !rows.is_empty() {
                    self.file_tree_cursor =
                        (self.file_tree_cursor + 1).min(rows.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.file_tree_cursor = self.file_tree_cursor.saturating_sub(1);
            }
            KeyCode::Enter => {
                // Jump to the file the cursor points at (skip directory rows)
                if let Some(row) = rows.get(self.file_tree_cursor) {
                    if let Some(file_idx) = row.file_index {
                        match self.detail_tab {
                            DetailTab::Diff => {
                                self.diff_file_cursor = file_idx;
                                self.diff_scroll = 0;
                            }
                            DetailTab::Difftastic => {
                                self.difft_file_cursor = file_idx;
                                self.difft_scroll = 0;
                            }
                            DetailTab::Comments => {}
                        }
                        // Move focus back to content after selecting
                        self.detail_focus = DetailFocus::Content;
                    }
                }
            }
            KeyCode::Char(' ') | KeyCode::Esc => {
                // Close tree / return focus to content
                self.detail_focus = DetailFocus::Content;
            }
            KeyCode::Tab => {
                self.detail_focus = DetailFocus::Content;
            }
            _ => {}
        }
        false
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
        }
    }

    // ── Async task launchers ──────────────────────────────────────────────────

    fn fetch_prs(&self) {
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
    fn max_diff_scroll(&self) -> u16 {
        let idx = self.diff_file_cursor.min(self.diff_files.len().saturating_sub(1));
        let total: usize = self.diff_files.get(idx).map(|f| {
            f.hunks.iter().map(|h| 1 + h.lines.len()).sum()
        }).unwrap_or(0);
        (total as u16).saturating_sub(1)
    }

    /// Total rendered lines for comments.
    fn max_comments_scroll(&self) -> u16 {
        // Each comment: 1 header line + body lines + 1 separator
        let total: usize = self.pr_comments.iter().map(|c| {
            1 + c.body.lines().count() + 1
        }).sum();
        (total as u16).saturating_sub(1)
    }

    /// Total rendered lines for the current difftastic file output.
    fn max_difft_scroll(&self) -> u16 {
        let idx = self.difft_file_cursor.min(self.difft_files.len().saturating_sub(1));
        let total = self.difft_files.get(idx)
            .map(|(_, raw)| raw.lines().count())
            .unwrap_or(0);
        (total as u16).saturating_sub(1)
    }

    // ── Actions ───────────────────────────────────────────────────────────────

    fn open_detail(&mut self) {
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
    fn toggle_reviewed(&mut self) {
        let Some(repo) = &self.repo else { return };
        let Some(pr) = self.prs.get(self.pr_cursor) else { return };
        let pr_number = pr.number;
        let owner = repo.owner.clone();
        let repo_name = repo.name.clone();

        let (file_cursor, total_files) = match self.detail_tab {
            DetailTab::Diff => (self.diff_file_cursor, self.diff_files.len()),
            DetailTab::Difftastic => (self.difft_file_cursor, self.difft_files.len()),
            DetailTab::Comments => return,
        };

        let filename = match self.detail_tab {
            DetailTab::Diff => self.diff_files.get(file_cursor).map(|f| f.filename.clone()),
            DetailTab::Difftastic => self.difft_files.get(file_cursor).map(|(n, _)| n.clone()),
            DetailTab::Comments => None,
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

    fn checkout_pr_branch(&mut self) {
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
        }
        let _ = config::save(&self.config);
    }
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
