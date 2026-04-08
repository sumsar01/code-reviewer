use anyhow::{Context, Result, bail};
use octocrab::Octocrab;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

/// Number of pull requests fetched per page when listing a repository's PRs.
const PR_LIST_PAGE_SIZE: u8 = 50;

/// Number of review-request search results fetched per page.
const REVIEW_REQUEST_PAGE_SIZE: u8 = 50;

/// Hard cap on GitHub repository search results per query.
const REPO_SEARCH_MAX_RESULTS: u8 = 10;

/// A lightweight repo search result.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RepoSearchResult {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub stars: u32,
}

/// A cross-repo PR where the authenticated user has been requested as a reviewer.
/// Sourced from GitHub's `/search/issues` endpoint (`is:pr is:open review-requested:ME`).
/// Lighter than `PullRequest` — no SHAs or file stats, because the search API
/// returns `Issue` objects rather than full PR objects.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ReviewRequestPr {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub url: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub draft: bool,
}

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
    octo: Octocrab,
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

            // Org match: check if the authenticated user is a member of the org
            // by querying their own membership. Uses /user/memberships/orgs/{org}
            // which returns 200 only when the token's user actually belongs to the org.
            let is_member = octo
                .get::<serde_json::Value, _, _>(
                    format!("/user/memberships/orgs/{repo_owner}"),
                    None::<&()>,
                )
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
            .per_page(PR_LIST_PAGE_SIZE)
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

    /// Fetch all open PRs across all repos where the authenticated user has been
    /// requested as a reviewer, using the GitHub search API.
    ///
    /// When `direct_only` is true (the default), uses `review-requested:@me` which
    /// matches only direct personal requests — team review requests are excluded.
    /// When false, uses `review-requested:<username>` which includes PRs requested
    /// via any GitHub team the user belongs to.
    pub async fn fetch_review_requested_prs(&self, direct_only: bool) -> Result<Vec<ReviewRequestPr>> {
        // GitHub's search API returns `Issue` objects for both issues and PRs.
        // The `pull_request` field is present (and non-null) when the item is a PR.
        #[derive(Deserialize)]
        struct SearchPage {
            items: Vec<SearchIssue>,
        }
        #[derive(Deserialize)]
        struct SearchIssue {
            number: u64,
            title: String,
            html_url: String,
            user: Option<SearchUser>,
            pull_request: Option<serde_json::Value>, // present iff it's a PR
            draft: Option<bool>,
            repository_url: String, // "https://api.github.com/repos/owner/name"
        }
        #[derive(Deserialize)]
        struct SearchUser {
            login: String,
        }

        let reviewer = if direct_only { "@me".to_string() } else { self.username.clone() };
        let query = format!("is:pr is:open review-requested:{reviewer}");
        let mut prs: Vec<ReviewRequestPr> = Vec::new();
        let mut page_num: u32 = 1;

        loop {
            let resp: SearchPage = self
                .octo
                .get(
                    "/search/issues",
                    Some(&[
                        ("q", query.as_str()),
                        ("per_page", &REVIEW_REQUEST_PAGE_SIZE.to_string()),
                        ("page", &page_num.to_string()),
                    ]),
                )
                .await
                .with_context(|| format!("Searching review-requested PRs for {}", self.username))?;

            let count = resp.items.len();

            for issue in resp.items {
                // Skip non-PRs (shouldn't happen given `is:pr`, but be safe)
                if issue.pull_request.is_none() {
                    continue;
                }

                // Parse owner/name from "https://api.github.com/repos/owner/name"
                let (repo_owner, repo_name) = parse_repository_url(&issue.repository_url);

                prs.push(ReviewRequestPr {
                    number: issue.number,
                    title: issue.title,
                    author: issue.user.map(|u| u.login).unwrap_or_default(),
                    url: issue.html_url,
                    repo_owner,
                    repo_name,
                    draft: issue.draft.unwrap_or(false),
                });
            }

            // Stop paginating when we received fewer items than the page size
            if count < REVIEW_REQUEST_PAGE_SIZE as usize {
                break;
            }
            page_num += 1;
        }

        Ok(prs)
    }

    /// Fetch a single PR by owner, repo, and number.
    /// Returns a fully-populated `PullRequest` (including head/base SHAs needed
    /// to open the diff view).
    pub async fn fetch_single_pr(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest> {
        let pr = self
            .octo
            .pulls(owner, repo)
            .get(number)
            .await
            .with_context(|| format!("Fetching PR #{number} for {owner}/{repo}"))?;

        let author = pr
            .user
            .as_ref()
            .map(|u| u.login.clone())
            .unwrap_or_default();

        Ok(PullRequest {
            number: pr.number,
            title: pr.title.clone().unwrap_or_else(|| "(no title)".to_string()),
            author,
            head_branch: pr.head.ref_field.clone(),
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
        })
    }

    /// Fetch the unified diff for a PR as a string.
    ///
    /// Uses lossy UTF-8 conversion so that PRs touching binary files (images,
    /// compiled artifacts, etc.) do not fail with `InvalidUtf8` – any
    /// non-UTF-8 bytes are replaced with the Unicode replacement character
    /// (U+FFFD) instead of returning an error.
    pub async fn pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<String> {
        use http::Method;
        use http_body_util::BodyExt as _;

        let route = format!("/repos/{owner}/{repo}/pulls/{pr_number}");
        let uri = http::Uri::builder()
            .path_and_query(route)
            .build()
            .context("building diff URI")?;

        let builder = http::request::Builder::new()
            .method(Method::GET)
            .uri(uri)
            .header(http::header::ACCEPT, "application/vnd.github.diff");

        let request = self
            .octo
            .build_request(builder, None::<&()>)
            .context("building diff request")?;

        let response = self
            .octo
            .execute(request)
            .await
            .with_context(|| format!("Fetching diff for PR #{pr_number}"))?;

        let body_bytes = response
            .into_body()
            .collect()
            .await
            .context("reading diff response body")?
            .to_bytes();

        Ok(String::from_utf8_lossy(&body_bytes).into_owned())
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

    /// Submit a pull-request review (approve / request changes / comment).
    ///
    /// `event` must be one of `"APPROVE"`, `"REQUEST_CHANGES"`, or `"COMMENT"`.
    /// `body` is the review comment text (required for REQUEST_CHANGES and COMMENT;
    /// may be empty for APPROVE).
    pub async fn submit_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        event: &str,
        body: &str,
    ) -> Result<()> {
        let payload = serde_json::json!({
            "event": event,
            "body": body,
        });

        self.octo
            .post::<serde_json::Value, serde_json::Value>(
                format!("/repos/{owner}/{repo}/pulls/{pr_number}/reviews"),
                Some(&payload),
            )
            .await
            .with_context(|| {
                format!("Submitting {event} review for PR #{pr_number} on {owner}/{repo}")
            })?;

        Ok(())
    }

    /// Post a single inline review comment on a specific diff line.
    ///
    /// * `commit_id` — the PR's head SHA (available as `PullRequest::head_sha`)
    /// * `path`      — file path relative to the repo root
    /// * `line`      — the line number on the chosen side
    /// * `side`      — `"LEFT"` for removed lines, `"RIGHT"` for added/context lines
    /// * `body`      — comment text
    pub async fn create_review_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        commit_id: &str,
        path: &str,
        line: u64,
        side: &str,
        body: &str,
    ) -> Result<()> {
        let payload = serde_json::json!({
            "body": body,
            "commit_id": commit_id,
            "path": path,
            "line": line,
            "side": side,
        });

        self.octo
            .post::<serde_json::Value, serde_json::Value>(
                format!("/repos/{owner}/{repo}/pulls/{pr_number}/comments"),
                Some(&payload),
            )
            .await
            .with_context(|| {
                format!(
                    "Posting inline comment on {path}:{line} ({side}) for PR #{pr_number} on {owner}/{repo}"
                )
            })?;

        Ok(())
    }

    /// Return the list of organisation login names the authenticated user belongs to.
    /// Returns an empty vec on any error (best-effort; never blocks the app).
    pub async fn fetch_user_orgs(&self) -> Vec<String> {
        #[derive(Deserialize)]
        struct OrgItem {
            login: String,
        }

        let result: Result<Vec<OrgItem>, _> = self
            .octo
            .get("/user/orgs", Some(&[("per_page", "100")]))
            .await;

        match result {
            Ok(orgs) => orgs.into_iter().map(|o| o.login).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Search GitHub repositories matching `query`.
    ///
    /// When `org_hints` is non-empty the query is biased so that repos owned
    /// by those orgs (or the authenticated user) rank higher in results.
    /// GitHub's search API supports `org:` qualifiers which limit OR-match to
    /// the supplied orgs without excluding global results when combined with a
    /// plain keyword — so we append them to the query string.
    ///
    /// Returns up to `per_page` results (max 10).
    pub async fn search_repos(
        &self,
        query: &str,
        per_page: u8,
        org_hints: &[String],
    ) -> Result<Vec<RepoSearchResult>> {
        #[derive(Deserialize)]
        struct SearchResponse {
            items: Vec<SearchItem>,
        }
        #[derive(Deserialize)]
        struct SearchItem {
            full_name: String,
            description: Option<String>,
            stargazers_count: u32,
        }

        // Build biased query: append "org:<name>" for each known org plus the
        // authenticated user so personal repos also bubble up.
        let biased_query = if org_hints.is_empty() {
            query.to_string()
        } else {
            let org_qualifiers: String = org_hints
                .iter()
                .map(|o| format!(" org:{o}"))
                .collect::<Vec<_>>()
                .join("");
            // Also include the authenticated user so personal repos rank highly.
            format!("{query} user:{}{org_qualifiers}", self.username)
        };

        let per_page = per_page.min(REPO_SEARCH_MAX_RESULTS);
        let resp: SearchResponse = self
            .octo
            .get(
                "/search/repositories",
                Some(&[
                    ("q", biased_query.as_str()),
                    ("per_page", &per_page.to_string()),
                    ("sort", "stars"),
                ]),
            )
            .await
            .with_context(|| format!("Searching repositories for {:?}", query))?;

        let results = resp
            .items
            .into_iter()
            .filter_map(|item| {
                let mut parts = item.full_name.splitn(2, '/');
                let owner = parts.next()?.to_string();
                let name = parts.next()?.to_string();
                Some(RepoSearchResult {
                    owner,
                    name,
                    description: item.description,
                    stars: item.stargazers_count,
                })
            })
            .collect();

        Ok(results)
    }

    /// Fetch CI check results for a specific commit SHA.
    /// Uses `statusCheckRollup { contexts }` which returns both modern CheckRuns
    /// (Aikido, GitHub Actions) and legacy StatusContexts (CircleCI) in one query.
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
            #[serde(rename = "statusCheckRollup")]
            status_check_rollup: Option<StatusCheckRollup>,
        }
        #[derive(Deserialize)]
        struct StatusCheckRollup {
            contexts: ContextConnection,
        }
        #[derive(Deserialize)]
        struct ContextConnection {
            nodes: Vec<ContextNode>,
        }
        #[derive(Deserialize)]
        #[serde(tag = "__typename")]
        enum ContextNode {
            CheckRun {
                name: String,
                status: String,
                conclusion: Option<String>,
                #[serde(rename = "startedAt")]
                started_at: Option<String>,
                #[serde(rename = "completedAt")]
                completed_at: Option<String>,
            },
            StatusContext {
                context: String,
                state: String,
            },
        }

        let query = r#"
            query($owner: String!, $repo: String!, $sha: GitObjectID!) {
              repository(owner: $owner, name: $repo) {
                object(oid: $sha) {
                  ... on Commit {
                    statusCheckRollup {
                      contexts(first: 100) {
                        nodes {
                          __typename
                          ... on CheckRun {
                            name
                            status
                            conclusion
                            startedAt
                            completedAt
                          }
                          ... on StatusContext {
                            context
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
            "variables": { "owner": owner, "repo": repo, "sha": head_sha }
        });

        let resp: Response = self
            .octo
            .graphql(&body)
            .await
            .with_context(|| format!("GraphQL statusCheckRollup query for {owner}/{repo}@{head_sha}"))?;

        let mut runs = Vec::new();
        if let Some(data) = resp.data {
            if let Some(repository) = data.repository {
                if let Some(obj) = repository.object {
                    if let Some(rollup) = obj.status_check_rollup {
                        for node in rollup.contexts.nodes {
                            match node {
                                ContextNode::CheckRun {
                                    name,
                                    status,
                                    conclusion,
                                    started_at,
                                    completed_at,
                                } => {
                                    runs.push(CheckRun {
                                        name,
                                        status,
                                        conclusion,
                                        started_at,
                                        completed_at,
                                    });
                                }
                                ContextNode::StatusContext { context, state } => {
                                    // Normalize legacy Status API state into CheckRun fields.
                                    // Strip common "ci/circleci: " prefix for cleaner names.
                                    let name = context
                                        .strip_prefix("ci/circleci: ")
                                        .unwrap_or(&context)
                                        .to_string();
                                    let (status, conclusion) = match state.to_uppercase().as_str() {
                                        "SUCCESS" => ("COMPLETED".to_string(), Some("SUCCESS".to_string())),
                                        "FAILURE" | "ERROR" => ("COMPLETED".to_string(), Some("FAILURE".to_string())),
                                        _ => ("IN_PROGRESS".to_string(), None), // PENDING
                                    };
                                    runs.push(CheckRun {
                                        name,
                                        status,
                                        conclusion,
                                        started_at: None,
                                        completed_at: None,
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

// ── GitHub URL helpers ────────────────────────────────────────────────────────

/// Parse `"https://api.github.com/repos/owner/name"` into `("owner", "name")`.
/// Returns empty strings if the URL doesn't match the expected format.
fn parse_repository_url(url: &str) -> (String, String) {
    // Strip any trailing slash, then take the last two path segments.
    let trimmed = url.trim_end_matches('/');
    let mut parts = trimmed.rsplitn(3, '/');
    let name = parts.next().unwrap_or("").to_string();
    let owner = parts.next().unwrap_or("").to_string();
    (owner, name)
}

// ── Diff parsing ──────────────────────────────────────────────────────────────

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
