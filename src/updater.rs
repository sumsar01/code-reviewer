//! Auto-update support: checks the GitHub Releases API for a newer version
//! and can perform a self-update via `cargo install --git`.
//!
//! The check is intentionally unauthenticated — it is a single GET request
//! per startup and comfortably within GitHub's 60 req/hr anonymous rate limit.

use std::process::Command;

/// Current version as set in `Cargo.toml`.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Repository URL as set in `Cargo.toml` (e.g. `https://github.com/owner/repo`).
const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Check the GitHub Releases API for a newer version.
///
/// Returns `Some(tag_name)` (e.g. `"v0.2.0"`) when a release with a tag
/// different from the current version is found.  Returns `None` when already
/// up to date, or on any network/parse error — the check is always silent on
/// failure so it never blocks startup.
pub async fn check_for_update() -> Option<String> {
    let (owner, repo) = parse_owner_repo(REPO_URL)?;
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/releases/latest"
    );

    let client = reqwest::Client::builder()
        .user_agent(format!("prr/{CURRENT_VERSION}"))
        .build()
        .ok()?;

    let response = client
        .get(&url)
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let body: serde_json::Value = response.json().await.ok()?;
    let tag = body.get("tag_name")?.as_str()?;

    // Strip a leading 'v' for the semver comparison.
    let tag_ver = tag.trim_start_matches('v');

    if tag_ver != CURRENT_VERSION {
        Some(tag.to_string())
    } else {
        None
    }
}

/// Shell out to `cargo install --git <repo> --tag <tag> --force` to perform
/// the update in-place.  This replaces the running binary; the user must
/// relaunch `prr` after the update completes.
///
/// Returns `Ok(())` on success, or `Err(String)` with a human-readable
/// description of what went wrong.
pub fn perform_update(tag: &str) -> Result<(), String> {
    let status = Command::new("cargo")
        .args([
            "install",
            "--git",
            REPO_URL,
            "--tag",
            tag,
            "--force",
        ])
        .status()
        .map_err(|e| format!("Failed to run cargo: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo install exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Extract `owner` and `repo` from a GitHub repository URL.
///
/// Handles both `https://github.com/owner/repo` and
/// `https://github.com/owner/repo.git` forms.
fn parse_owner_repo(url: &str) -> Option<(&str, &str)> {
    // Strip scheme and host: keep only the path component.
    let path = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .split("github.com/")
        .nth(1)?;

    let mut parts = path.splitn(2, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_owner_repo() {
        assert_eq!(
            parse_owner_repo("https://github.com/sumsar01/code-reviewer"),
            Some(("sumsar01", "code-reviewer"))
        );
        assert_eq!(
            parse_owner_repo("https://github.com/sumsar01/code-reviewer.git"),
            Some(("sumsar01", "code-reviewer"))
        );
        assert_eq!(
            parse_owner_repo("https://github.com/sumsar01/code-reviewer/"),
            Some(("sumsar01", "code-reviewer"))
        );
        assert_eq!(parse_owner_repo("not-a-github-url"), None);
    }
}
