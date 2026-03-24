use anyhow::{bail, Context, Result};
use git2::Repository;

/// Detected GitHub repository coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    pub owner: String,
    pub name: String,
}

impl RepoInfo {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Open the git repo rooted at `path` and extract the GitHub owner/repo from
/// the first remote whose URL contains "github.com".
pub fn detect_repo(path: &std::path::Path) -> Result<RepoInfo> {
    let repo = Repository::discover(path).context("Not inside a git repository")?;

    let remotes = repo.remotes()?;
    for name in remotes.iter().flatten() {
        let remote = repo.find_remote(name)?;
        if let Some(url) = remote.url() {
            if let Some(info) = parse_github_url(url) {
                return Ok(info);
            }
        }
    }

    bail!("No GitHub remote found in this repository")
}

/// Parse owner/repo out of various GitHub URL formats:
///   https://github.com/owner/repo.git
///   git@github.com:owner/repo.git
fn parse_github_url(url: &str) -> Option<RepoInfo> {
    // HTTPS: https://github.com/owner/repo[.git]
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        return split_owner_repo(rest);
    }

    // SSH: git@github.com:owner/repo[.git]
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return split_owner_repo(rest);
    }

    None
}

fn split_owner_repo(s: &str) -> Option<RepoInfo> {
    // Remove trailing .git
    let s = s.strip_suffix(".git").unwrap_or(s);
    let mut parts = s.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(RepoInfo { owner, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_with_git_suffix() {
        let r = parse_github_url("https://github.com/sumsar01/prr.git").unwrap();
        assert_eq!(r.owner, "sumsar01");
        assert_eq!(r.name, "prr");
    }

    #[test]
    fn https_without_git_suffix() {
        let r = parse_github_url("https://github.com/sumsar01/prr").unwrap();
        assert_eq!(r.owner, "sumsar01");
        assert_eq!(r.name, "prr");
    }

    #[test]
    fn ssh_url() {
        let r = parse_github_url("git@github.com:sumsar01/prr.git").unwrap();
        assert_eq!(r.owner, "sumsar01");
        assert_eq!(r.name, "prr");
    }

    #[test]
    fn non_github_returns_none() {
        assert!(parse_github_url("https://gitlab.com/foo/bar.git").is_none());
    }
}
