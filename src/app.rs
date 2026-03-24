use crate::config::{self, Config};
use crate::git::{self, RepoInfo};
use crate::github::{DiffFile, GitHubClient, PullRequest, ReviewComment, parse_diff};
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
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
    DiffLoaded(Vec<DiffFile>),
    DiffError(String),
    CommentsLoaded(Vec<ReviewComment>),
    CommentsError(String),
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub detail_tab: DetailTab,
    pub show_help: bool,
    pub config: Config,

    pub repo: Option<RepoInfo>,
    pub github: Arc<GitHubClient>,

    pub prs: Vec<PullRequest>,
    pub pr_cursor: usize,
    pub pr_load_state: LoadState,

    pub diff_files: Vec<DiffFile>,
    pub diff_scroll: u16,
    pub diff_file_cursor: usize,
    pub diff_load_state: LoadState,

    pub pr_comments: Vec<ReviewComment>,
    pub comments_scroll: u16,
    pub comments_load_state: LoadState,

    tx: mpsc::UnboundedSender<BgMsg>,
    rx: mpsc::UnboundedReceiver<BgMsg>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = config::load()?;
        let github = Arc::new(GitHubClient::new().await?);

        // Detect CWD repo
        let repo = git::detect_repo(&std::env::current_dir()?)
            .ok()
            // fall back to config's last repo
            .or_else(|| {
                config.ui.last_repo.as_ref().and_then(|s| {
                    let mut parts = s.splitn(2, '/');
                    Some(RepoInfo {
                        owner: parts.next()?.to_string(),
                        name: parts.next()?.to_string(),
                    })
                })
            });

        let (tx, rx) = mpsc::unbounded_channel();

        let app = Self {
            screen: Screen::PrList,
            detail_tab: DetailTab::Diff,
            show_help: false,
            config,
            repo,
            github,
            prs: Vec::new(),
            pr_cursor: 0,
            pr_load_state: LoadState::Loading,
            diff_files: Vec::new(),
            diff_scroll: 0,
            diff_file_cursor: 0,
            diff_load_state: LoadState::Idle,
            pr_comments: Vec::new(),
            comments_scroll: 0,
            comments_load_state: LoadState::Idle,
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
                        if self.handle_key(key.code) {
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
    fn handle_key(&mut self, code: KeyCode) -> bool {
        // Help overlay intercepts everything except ?
        if self.show_help {
            self.show_help = false;
            return false;
        }

        match &self.screen {
            Screen::PrList => self.handle_key_list(code),
            Screen::PrDetail => self.handle_key_detail(code),
        }
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
            _ => {}
        }
        false
    }

    fn handle_key_detail(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = Screen::PrList;
                self.diff_scroll = 0;
                self.comments_scroll = 0;
                self.diff_file_cursor = 0;
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Tab => {
                self.detail_tab = match self.detail_tab {
                    DetailTab::Diff => DetailTab::Comments,
                    DetailTab::Comments => DetailTab::Diff,
                };
            }
            KeyCode::Char('j') | KeyCode::Down => match self.detail_tab {
                DetailTab::Diff => self.diff_scroll = self.diff_scroll.saturating_add(1),
                DetailTab::Comments => self.comments_scroll = self.comments_scroll.saturating_add(1),
            },
            KeyCode::Char('k') | KeyCode::Up => match self.detail_tab {
                DetailTab::Diff => self.diff_scroll = self.diff_scroll.saturating_sub(1),
                DetailTab::Comments => self.comments_scroll = self.comments_scroll.saturating_sub(1),
            },
            KeyCode::Char('n') => {
                if !self.diff_files.is_empty() {
                    self.diff_file_cursor =
                        (self.diff_file_cursor + 1).min(self.diff_files.len() - 1);
                    self.diff_scroll = 0;
                }
            }
            KeyCode::Char('N') => {
                self.diff_file_cursor = self.diff_file_cursor.saturating_sub(1);
                self.diff_scroll = 0;
            }
            KeyCode::Char('o') => {
                if let Some(pr) = self.prs.get(self.pr_cursor) {
                    let _ = open::that(&pr.url);
                }
            }
            KeyCode::Char('c') => {
                self.checkout_pr_branch();
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
            }
            BgMsg::PrsError(e) => {
                self.pr_load_state = LoadState::Error(e);
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
                Err(e) => { let _ = tx.send(BgMsg::PrsError(e.to_string())); }
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
                Err(e) => { let _ = tx.send(BgMsg::DiffError(e.to_string())); }
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
                Err(e) => { let _ = tx.send(BgMsg::CommentsError(e.to_string())); }
            }
        });
    }

    // ── Actions ───────────────────────────────────────────────────────────────

    fn open_detail(&mut self) {
        let Some(pr) = self.prs.get(self.pr_cursor) else { return };
        let pr_number = pr.number;

        self.screen = Screen::PrDetail;
        self.detail_tab = DetailTab::Diff;
        self.diff_files = Vec::new();
        self.diff_load_state = LoadState::Loading;
        self.pr_comments = Vec::new();
        self.comments_load_state = LoadState::Loading;

        self.fetch_diff(pr_number);
        self.fetch_comments(pr_number);
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
