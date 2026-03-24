use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// The last repo used (owner/name), restored on next run.
    pub last_repo: Option<String>,
    /// If true, show all PRs; if false, show only the authenticated user's PRs.
    pub show_all_prs: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            last_repo: None,
            show_all_prs: false,
        }
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
