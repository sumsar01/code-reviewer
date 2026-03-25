//! `code_reviewer` library — programmatic access to GitHub PR data.
//!
//! Exposes the GitHub client, git repo detection, config, and error types
//! so other tools can list, diff, and comment on pull requests using the
//! current user's `gh` CLI authentication.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use code_reviewer::git::detect_repo;
//! use code_reviewer::github::{GitHubClient, parse_diff};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let repo = detect_repo(std::env::current_dir()?.as_path())?;
//! let client = GitHubClient::new_for_owner(&repo.owner).await?;
//!
//! let prs = client.list_prs(&repo.owner, &repo.name, false).await?;
//! let raw_diff = client.pr_diff(&repo.owner, &repo.name, prs[0].number).await?;
//! let diff_files = parse_diff(&raw_diff);
//! let comments = client.pr_comments(&repo.owner, &repo.name, prs[0].number).await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod git;
pub mod github;
