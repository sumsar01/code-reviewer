use anyhow::{Context, Result, bail};
use octocrab::Octocrab;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

/// A GitHub pull request (subset of fields we care about).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub head_branch: String,
    pub base_branch: String,
    pub head_sha: String,
    pub base_sha: String,
    pub state: String,
    pub draft: bool,
    pub url: String,
    pub additions: Option<i64>,
    pub deletions: Option<i64>,
    pub changed_files: Option<i64>,
    pub review_decision: Option<String>,
    pub ci_status: Option<String>,
    pub body: Option<String>,
}

/// A single review comment / thread item.
#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub author: String,
    pub body: String,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub created_at: String,
}

/// A parsed diff file section.
#[derive(Debug, Clone)]
pub struct DiffFile {
    pub filename: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub left_no: Option<usize>,
    pub right_no: Option<usize>,
    pub content: String,
}

/// Combined per-PR metadata fetched in one GraphQL round-trip.
pub struct PrMetadata {
    pub review_decisions: HashMap<u64, String>,
    pub ci_statuses: HashMap<u64, String>,
}

/// A single CI check run on a commit.
#[derive(Debug, Clone)]
pub struct CheckRun {
    pub name: String,
    /// "QUEUED" | "IN_PROGRESS" | "COMPLETED"
    pub status: String,
    /// "SUCCESS" | "FAILURE" | "SKIPPED" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | None
    pub conclusion: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

pub struct GitHubClient {
    pub octo: Octocrab,
    pub username: String,
}

impl GitHubClient {
    /// Build a client, picking the gh account that can access `repo_owner`.
    /// Falls back to the active account if no better match is found.
    pub async fn new_for_owner(repo_owner: &str) -> Result<Self> {
        let accounts = list_gh_accounts()?;

        for account in &accounts {
            let token = get_gh_token_for(account)?;
            let octo = Octocrab::builder()
                .personal_token(token.clone())
                .build()
                .context("Building octocrab client")?;

            let user = octo.current().user().await;
            let username = match user {
                Ok(u) => u.login,
                Err(_) => continue,
            };

            // Exact match: user owns the repo directly
            if username.eq_ignore_ascii_case(repo_owner) {
                return Ok(Self { octo, username });
            }

            // Org match: check if repo_owner is an org this account belongs to
            let is_member = octo
                .orgs(repo_owner)
                .check_membership(&username)
                .await
                .is_ok();

            if is_member {
                return Ok(Self { octo, username });
            }
        }

        // Fall back to active account
        let token = get_gh_token()?;
        let octo = Octocrab::builder()
            .personal_token(token)
            .build()
            .context("Building octocrab client")?;
        let user = octo
            .current()
            .user()
            .await
            .context("Fetching authenticated user")?;
        Ok(Self { octo, username: user.login })
    }

    /// List open PRs for the given repo.
    /// If `mine_only` is true, filters to PRs authored by the authenticated user.
    pub async fn list_prs(
        &self,
        owner: &str,
        repo: &str,
        mine_only: bool,
    ) -> Result<Vec<PullRequest>> {
        let mut page = self
            .octo
            .pulls(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(50)
            .send()
            .await
            .with_context(|| format!("Listing PRs for {owner}/{repo}"))?;

        let mut prs: Vec<PullRequest> = Vec::new();

        loop {
            for pr in &page.items {
                let author = pr
                    .user
                    .as_ref()
                    .map(|u| u.login.clone())
                    .unwrap_or_default();

                if mine_only && author != self.username {
                    continue;
                }

                prs.push(PullRequest {
                    number: pr.number,
                    title: pr
                        .title
                        .clone()
                        .unwrap_or_else(|| "(no title)".to_string()),
                    author,
                    head_branch: pr
                        .head
                        .ref_field
                        .clone(),
                    base_branch: pr.base.ref_field.clone(),
                    head_sha: pr.head.sha.clone(),
                    base_sha: pr.base.sha.clone(),
                    state: pr
                        .state
                        .as_ref()
                        .map(|s| format!("{:?}", s).to_lowercase())
                        .unwrap_or_default(),
                    draft: pr.draft.unwrap_or(false),
                    url: pr
                        .html_url
                        .as_ref()
                        .map(|u| u.to_string())
                        .unwrap_or_default(),
                    additions: pr.additions.map(|v| v as i64),
                    deletions: pr.deletions.map(|v| v as i64),
                    changed_files: pr.changed_files.map(|v| v as i64),
                    review_decision: None,
                    ci_status: None,
                    body: pr.body.clone(),
                });
            }

            match self.octo.get_page::<octocrab::models::pulls::PullRequest>(&page.next).await? {
                Some(next) => page = next,
                None => break,
            }
        }

        Ok(prs)
    }

    /// Fetch the unified diff for a PR as a string.
    pub async fn pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<String> {
        self.octo
            .pulls(owner, repo)
            .get_diff(pr_number)
            .await
            .with_context(|| format!("Fetching diff for PR #{pr_number}"))
    }

    /// Fetch review comments for a PR.
    pub async fn pr_comments(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<ReviewComment>> {
        let comments = self
            .octo
            .pulls(owner, repo)
            .list_comments(Some(pr_number))
            .per_page(100)
            .send()
            .await
            .with_context(|| format!("Fetching comments for PR #{pr_number}"))?;

        let result = comments
            .items
            .into_iter()
            .map(|c| ReviewComment {
                author: c.user.as_ref().map(|u| u.login.clone()).unwrap_or_default(),
                body: c.body.clone(),
                path: Some(c.path.clone()),
                line: c.line,
                created_at: c.created_at.to_rfc3339(),
            })
            .collect();

        Ok(result)
    }

    /// Fetch `reviewDecision` and CI `statusCheckRollup` for all open PRs in one
    /// GraphQL request.
    ///
    /// Returns a `PrMetadata` with maps from PR number → review decision string
    /// (`"APPROVED"`, `"CHANGES_REQUESTED"`, `"REVIEW_REQUIRED"`, or absent when `null`)
    /// and PR number → CI state string (`"SUCCESS"`, `"FAILURE"`, `"PENDING"`,
    /// `"ERROR"`, `"EXPECTED"`, or absent when there are no checks).
    pub async fn fetch_review_decisions(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<PrMetadata> {
        // GraphQL response shapes
        #[derive(Deserialize)]
        struct Response {
            data: Option<Data>,
        }
        #[derive(Deserialize)]
        struct Data {
            repository: Option<Repository>,
        }
        #[derive(Deserialize)]
        struct Repository {
            #[serde(rename = "pullRequests")]
            pull_requests: Connection,
        }
        #[derive(Deserialize)]
        struct Connection {
            nodes: Vec<Node>,
        }
        #[derive(Deserialize)]
        struct Node {
            number: u64,
            #[serde(rename = "reviewDecision")]
            review_decision: Option<String>,
            commits: CommitConnection,
        }
        #[derive(Deserialize)]
        struct CommitConnection {
            nodes: Vec<CommitNode>,
        }
        #[derive(Deserialize)]
        struct CommitNode {
            commit: CommitObj,
        }
        #[derive(Deserialize)]
        struct CommitObj {
            #[serde(rename = "statusCheckRollup")]
            status_check_rollup: Option<StatusRollup>,
        }
        #[derive(Deserialize)]
        struct StatusRollup {
            state: String,
        }

        let query = r#"
            query($owner: String!, $repo: String!) {
              repository(owner: $owner, name: $repo) {
                pullRequests(states: OPEN, first: 100) {
                  nodes {
                    number
                    reviewDecision
                    commits(last: 1) {
                      nodes {
                        commit {
                          statusCheckRollup {
                            state
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
        "#;

        let body = serde_json::json!({
            "query": query,
            "variables": { "owner": owner, "repo": repo }
        });

        let resp: Response = self
            .octo
            .graphql(&body)
            .await
            .with_context(|| format!("GraphQL reviewDecision+CI query for {owner}/{repo}"))?;

        let mut review_decisions = HashMap::new();
        let mut ci_statuses = HashMap::new();
        if let Some(data) = resp.data {
            if let Some(repository) = data.repository {
                for node in repository.pull_requests.nodes {
                    if let Some(decision) = node.review_decision {
                        review_decisions.insert(node.number, decision);
                    }
                    if let Some(ci_state) = node
                        .commits
                        .nodes
                        .first()
                        .and_then(|cn| cn.commit.status_check_rollup.as_ref())
                        .map(|r| r.state.clone())
                    {
                        ci_statuses.insert(node.number, ci_state);
                    }
                }
            }
        }
        Ok(PrMetadata { review_decisions, ci_statuses })
    }

    /// Fetch individual CI check runs for a specific commit SHA.
    /// Returns a flat list of check runs across all check suites.
    pub async fn fetch_check_runs(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<CheckRun>> {
        #[derive(Deserialize)]
        struct Response {
            data: Option<Data>,
        }
        #[derive(Deserialize)]
        struct Data {
            repository: Option<Repository>,
        }
        #[derive(Deserialize)]
        struct Repository {
            object: Option<GitObject>,
        }
        #[derive(Deserialize)]
        struct GitObject {
            #[serde(rename = "checkSuites")]
            check_suites: Option<CheckSuiteConnection>,
        }
        #[derive(Deserialize)]
        struct CheckSuiteConnection {
            nodes: Vec<CheckSuiteNode>,
        }
        #[derive(Deserialize)]
        struct CheckSuiteNode {
            #[serde(rename = "checkRuns")]
            check_runs: Option<CheckRunConnection>,
        }
        #[derive(Deserialize)]
        struct CheckRunConnection {
            nodes: Vec<CheckRunNode>,
        }
        #[derive(Deserialize)]
        struct CheckRunNode {
            name: String,
            status: String,
            conclusion: Option<String>,
            #[serde(rename = "startedAt")]
            started_at: Option<String>,
            #[serde(rename = "completedAt")]
            completed_at: Option<String>,
        }

        let query = r#"
            query($owner: String!, $repo: String!, $sha: String!) {
              repository(owner: $owner, name: $repo) {
                object(expression: $sha) {
                  ... on Commit {
                    checkSuites(first: 20) {
                      nodes {
                        checkRuns(first: 50) {
                          nodes {
                            name
                            status
                            conclusion
                            startedAt
                            completedAt
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
        "#;

        let body = serde_json::json!({
            "query": query,
            "variables": { "owner": owner, "repo": repo, "sha": head_sha }
        });

        let resp: Response = self
            .octo
            .graphql(&body)
            .await
            .with_context(|| format!("GraphQL checkRuns query for {owner}/{repo}@{head_sha}"))?;

        let mut runs = Vec::new();
        if let Some(data) = resp.data {
            if let Some(repository) = data.repository {
                if let Some(obj) = repository.object {
                    if let Some(suites) = obj.check_suites {
                        for suite in suites.nodes {
                            if let Some(check_runs) = suite.check_runs {
                                for run in check_runs.nodes {
                                    runs.push(CheckRun {
                                        name: run.name,
                                        status: run.status,
                                        conclusion: run.conclusion,
                                        started_at: run.started_at,
                                        completed_at: run.completed_at,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(runs)
    }
}

fn get_gh_token() -> Result<String> {
    gh_token_cmd(&["auth", "token"])
}

/// Get the GitHub OAuth token for a specific gh account username.
fn get_gh_token_for(user: &str) -> Result<String> {
    gh_token_cmd(&["auth", "token", "-u", user])
}

fn gh_token_cmd(args: &[&str]) -> Result<String> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .context("Running `gh auth token` — is the gh CLI installed?")?;

    if !output.status.success() {
        bail!(
            "gh auth token failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let token = String::from_utf8(output.stdout)
        .context("gh token output is not valid UTF-8")?
        .trim()
        .to_string();

    if token.is_empty() {
        bail!("gh auth token returned an empty token — run `gh auth login`");
    }

    Ok(token)
}

/// Return all account logins from `gh auth status`.
fn list_gh_accounts() -> Result<Vec<String>> {
    let output = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .context("Running `gh auth status`")?;

    // gh auth status may write to stdout (gh >= 2.x) or stderr (older gh).
    // Combine both to be safe across versions.
    let combined: Vec<u8> = output.stdout.iter().chain(output.stderr.iter()).copied().collect();
    let text = String::from_utf8_lossy(&combined);
    let mut accounts = Vec::new();
    for line in text.lines() {
        // Lines look like:  "  ✓ Logged in to github.com account USERNAME (keyring)"
        if let Some(rest) = line.trim().strip_prefix("✓ Logged in to github.com account ") {
            let username = rest.split_whitespace().next().unwrap_or("").to_string();
            if !username.is_empty() {
                accounts.push(username);
            }
        }
    }
    Ok(accounts)
}

// ── Diff parsing ─────────────────────────────────────────────────────────────

pub fn parse_diff(raw: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut cur_file: Option<DiffFile> = None;
    let mut cur_hunk: Option<DiffHunk> = None;
    let mut left_no: usize = 0;
    let mut right_no: usize = 0;

    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            flush_hunk(&mut cur_hunk, &mut cur_file);
            flush_file(&mut cur_file, &mut files);
            cur_file = Some(DiffFile {
                filename: extract_filename(line),
                hunks: Vec::new(),
            });
        } else if line.starts_with("--- ") || line.starts_with("+++ ") || line.starts_with("index ") || line.starts_with("new file") || line.starts_with("deleted file") || line.starts_with("Binary") {
            // skip meta lines
        } else if line.starts_with("@@ ") {
            flush_hunk(&mut cur_hunk, &mut cur_file);
            // Parse @@ -l,s +l,s @@ header
            let (l, r) = parse_hunk_header(line);
            left_no = l;
            right_no = r;
            cur_hunk = Some(DiffHunk {
                header: line.to_string(),
                lines: Vec::new(),
            });
        } else if let Some(hunk) = cur_hunk.as_mut() {
            let (kind, content) = if let Some(rest) = line.strip_prefix('+') {
                let ln = right_no;
                right_no += 1;
                (DiffLineKind::Added, (None, Some(ln), rest.to_string()))
            } else if let Some(rest) = line.strip_prefix('-') {
                let ln = left_no;
                left_no += 1;
                (DiffLineKind::Removed, (Some(ln), None, rest.to_string()))
            } else {
                let rest = line.strip_prefix(' ').unwrap_or(line);
                let (ll, rl) = (left_no, right_no);
                left_no += 1;
                right_no += 1;
                (DiffLineKind::Context, (Some(ll), Some(rl), rest.to_string()))
            };
            hunk.lines.push(DiffLine {
                kind,
                left_no: content.0,
                right_no: content.1,
                content: content.2,
            });
        }
    }

    flush_hunk(&mut cur_hunk, &mut cur_file);
    flush_file(&mut cur_file, &mut files);
    files
}

fn flush_hunk(hunk: &mut Option<DiffHunk>, file: &mut Option<DiffFile>) {
    if let (Some(h), Some(f)) = (hunk.take(), file.as_mut()) {
        f.hunks.push(h);
    }
}

fn flush_file(file: &mut Option<DiffFile>, files: &mut Vec<DiffFile>) {
    if let Some(f) = file.take() {
        files.push(f);
    }
}

fn extract_filename(diff_git_line: &str) -> String {
    // "diff --git a/foo/bar.rs b/foo/bar.rs"
    diff_git_line
        .split_whitespace()
        .last()
        .and_then(|s| s.strip_prefix("b/"))
        .unwrap_or(diff_git_line)
        .to_string()
}

fn parse_hunk_header(header: &str) -> (usize, usize) {
    // @@ -l,s +l,s @@
    let mut left = 1usize;
    let mut right = 1usize;
    if let Some(inner) = header.strip_prefix("@@ ") {
        let parts: Vec<&str> = inner.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Some(l) = parts[0].strip_prefix('-') {
                left = l.split(',').next().and_then(|n| n.parse().ok()).unwrap_or(1);
            }
            if let Some(r) = parts[1].strip_prefix('+') {
                right = r.split(',').next().and_then(|n| n.parse().ok()).unwrap_or(1);
            }
        }
    }
    (left, right)
}
