use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Auth error: gh CLI not found or not authenticated. Run `gh auth login` first.")]
    Auth,

    #[error("Not a GitHub repository: {0}")]
    NotGitHub(String),
}
