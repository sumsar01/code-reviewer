use crate::config::{self, Config};
use crate::git::{self, RepoInfo};
use crate::github::{DiffFile, GitHubClient, PullRequest, ReviewComment, parse_diff};
use crate::ui;
use crate::ui::theme::{Theme, ALL_THEMES};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{Terminal, backend::CrosstermBackend, text::Line};
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
    Difftastic,
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
    DifftLoaded(Vec<(String, Vec<Line<'static>>)>),
    DifftError(String),
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub detail_tab: DetailTab,
    pub show_help: bool,
    pub config: Config,
    pub theme: Theme,

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

    pub difft_files: Vec<(String, Vec<Line<'static>>)>, // (filename, ansi lines)
    pub difft_scroll: u16,
    pub difft_file_cursor: usize,
    pub difft_load_state: LoadState,

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
            show_help: false,
            theme,
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
            difft_files: Vec::new(),
            difft_scroll: 0,
            difft_file_cursor: 0,
            difft_load_state: LoadState::Idle,
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
                    DetailTab::Comments => DetailTab::Difftastic,
                    DetailTab::Difftastic => DetailTab::Diff,
                };
            }
            KeyCode::Char('j') | KeyCode::Down => match self.detail_tab {
                DetailTab::Diff => {
                    let max = self.max_diff_scroll();
                    self.diff_scroll = self.diff_scroll.saturating_add(1).min(max);
                }
                DetailTab::Comments => {
                    let max = self.max_comments_scroll();
                    self.comments_scroll = self.comments_scroll.saturating_add(1).min(max);
                }
                DetailTab::Difftastic => {
                    let max = self.max_difft_scroll();
                    self.difft_scroll = self.difft_scroll.saturating_add(1).min(max);
                }
            },
            KeyCode::Char('k') | KeyCode::Up => match self.detail_tab {
                DetailTab::Diff => self.diff_scroll = self.diff_scroll.saturating_sub(1),
                DetailTab::Comments => self.comments_scroll = self.comments_scroll.saturating_sub(1),
                DetailTab::Difftastic => self.difft_scroll = self.difft_scroll.saturating_sub(1),
            },
            KeyCode::Char('n') => {
                match self.detail_tab {
                    DetailTab::Difftastic => {
                        if !self.difft_files.is_empty() {
                            self.difft_file_cursor =
                                (self.difft_file_cursor + 1).min(self.difft_files.len() - 1);
                            self.difft_scroll = 0;
                        }
                    }
                    _ => {
                        if !self.diff_files.is_empty() {
                            self.diff_file_cursor =
                                (self.diff_file_cursor + 1).min(self.diff_files.len() - 1);
                            self.diff_scroll = 0;
                        }
                    }
                }
            }
            KeyCode::Char('N') => {
                match self.detail_tab {
                    DetailTab::Difftastic => {
                        self.difft_file_cursor = self.difft_file_cursor.saturating_sub(1);
                        self.difft_scroll = 0;
                    }
                    _ => {
                        self.diff_file_cursor = self.diff_file_cursor.saturating_sub(1);
                        self.diff_scroll = 0;
                    }
                }
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
            BgMsg::DifftLoaded(files) => {
                self.difft_files = files;
                self.difft_load_state = LoadState::Idle;
                self.difft_file_cursor = 0;
                self.difft_scroll = 0;
            }
            BgMsg::DifftError(e) => {
                self.difft_load_state = LoadState::Error(e);
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
                    "diff.external=difft",
                    "diff",
                    "--ext-diff",
                    "--color=always",
                    &format!("{}..{}", base_sha, head_sha),
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
        let total = self.difft_files.get(idx).map(|(_, lines)| lines.len()).unwrap_or(0);
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

        self.fetch_diff(pr_number);
        self.fetch_comments(pr_number);
        self.fetch_difft(base_sha, head_sha);
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

/// Split the combined `git diff --ext-diff` output (where each file's difft
/// output is concatenated) into per-file sections.
///
/// Difftastic starts each file with a header line of the form:
///   `<ANSI codes><filename><ANSI reset> --- <Language>`
///
/// We detect these headers by stripping ANSI escape sequences and matching
/// the ` --- ` separator that cannot appear in normal diff content lines
/// (line numbers, code, etc.).
fn split_difft_output(stdout: &str) -> Vec<(String, Vec<Line<'static>>)> {
    let mut results: Vec<(String, Vec<Line<'static>>)> = Vec::new();

    // Each "section" is (filename, raw lines of that section including header).
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in stdout.lines() {
        let stripped = strip_ansi(line);
        // A file header looks like "src/foo.rs --- Rust" or ".gitignore --- Text"
        // It must not start with a digit (line numbers) and must contain " --- ".
        if let Some(filename) = detect_file_header(&stripped) {
            // Flush previous section
            if let Some(name) = current_name.take() {
                let parsed = crate::ui::difftastic::parse_ansi_to_lines(
                    &current_lines.join("\n"),
                );
                results.push((name, parsed));
                current_lines.clear();
            }
            current_name = Some(filename);
            current_lines.push(line);
        } else {
            current_lines.push(line);
        }
    }

    // Flush last section
    if let Some(name) = current_name.take() {
        let parsed = crate::ui::difftastic::parse_ansi_to_lines(
            &current_lines.join("\n"),
        );
        results.push((name, parsed));
    }

    results
}

/// Detect whether a (ANSI-stripped) line is a difftastic file header.
/// Returns the filename if it is, or `None` otherwise.
///
/// Header format: `<path> --- <Language>`
/// Rules to avoid false positives:
///   - Must contain " --- "
///   - The part before " --- " must not be empty
///   - Must not start with a digit (would be a line-number column)
fn detect_file_header(stripped: &str) -> Option<String> {
    let sep = " --- ";
    let pos = stripped.find(sep)?;
    let before = &stripped[..pos];
    let after = &stripped[pos + sep.len()..];

    // Must have a non-empty path and a non-empty language
    if before.is_empty() || after.is_empty() {
        return None;
    }

    // Must not start with a digit (line-number columns from difft output)
    if before.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    // Language label should be a single word or short identifier (no spaces
    // containing digit-only strings = avoids matching diff content).
    // Just check that the language part doesn't start with a digit.
    let lang_word = after.split_whitespace().next().unwrap_or("");
    if lang_word.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    Some(before.to_string())
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
            // Safe: we only advance by one byte when it's ASCII; for multi-byte
            // UTF-8 sequences we need to push the whole char.
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
