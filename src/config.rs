use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    /// Reviewed file state, keyed by "owner/repo/pr_number/filename".
    /// A `true` value means the file has been marked as reviewed.
    #[serde(default)]
    pub reviewed_files: HashMap<String, bool>,
}

impl Config {
    /// Build the config key for a specific file in a PR.
    pub fn reviewed_key(owner: &str, repo: &str, pr_number: u64, filename: &str) -> String {
        format!("{owner}/{repo}/{pr_number}/{filename}")
    }

    /// Returns `true` if the given file has been marked as reviewed.
    pub fn is_reviewed(&self, owner: &str, repo: &str, pr_number: u64, filename: &str) -> bool {
        let key = Self::reviewed_key(owner, repo, pr_number, filename);
        self.reviewed_files.get(&key).copied().unwrap_or(false)
    }

    /// Toggles the reviewed state of a file. Returns the new state.
    pub fn toggle_reviewed(
        &mut self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        filename: &str,
    ) -> bool {
        let key = Self::reviewed_key(owner, repo, pr_number, filename);
        let current = self.reviewed_files.get(&key).copied().unwrap_or(false);
        let new_state = !current;
        self.reviewed_files.insert(key, new_state);
        new_state
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// The last repo used (owner/name), restored on next run.
    pub last_repo: Option<String>,
    /// If true, show all PRs; if false, show only the authenticated user's PRs.
    pub show_all_prs: bool,
    /// Name of the color theme to use. Defaults to "tokyonight".
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Recently visited repos (owner/name), most recent first. Capped at 10.
    #[serde(default)]
    pub recent_repos: Vec<String>,
    /// Hide PRs authored by Dependabot from the review-requests screen.
    #[serde(default)]
    pub review_request_hide_dependabot: bool,
    /// Hide PRs that have not been updated in more than this many days.
    /// Set to 0 to disable. Defaults to 30.
    #[serde(default = "default_review_request_max_age_days")]
    pub review_request_max_age_days: u64,
}

fn default_theme() -> String {
    "tokyonight".to_string()
}

fn default_review_request_max_age_days() -> u64 {
    30
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            last_repo: None,
            show_all_prs: false,
            theme: default_theme(),
            recent_repos: Vec::new(),
            review_request_hide_dependabot: false,
            review_request_max_age_days: default_review_request_max_age_days(),
        }
    }
}

impl UiConfig {
    /// Prepend `repo` (as "owner/name") to `recent_repos`, deduplicating and
    /// capping the list at 10 entries.
    pub fn push_recent_repo(&mut self, owner: &str, name: &str) {
        let full = format!("{owner}/{name}");
        self.recent_repos.retain(|r| r != &full);
        self.recent_repos.insert(0, full);
        self.recent_repos.truncate(10);
    }
}

fn config_path() -> Result<PathBuf> {
    let base = dirs_next::config_dir().context("Could not determine config directory")?;
    Ok(base.join("prr").join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Reading config from {}", path.display()))?;
    toml::from_str(&contents).context("Parsing config.toml")
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Creating config dir {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(config).context("Serialising config")?;
    fs::write(&path, contents).with_context(|| format!("Writing config to {}", path.display()))
}
