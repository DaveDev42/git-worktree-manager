/// Custom error types for git-worktree-manager.
///
/// Maps directly from the Python exception hierarchy in exceptions.py.
use thiserror::Error;

/// Base error type for all git-worktree-manager errors.
#[derive(Error, Debug)]
pub enum CwError {
    /// Raised when a git operation fails.
    #[error("{0}")]
    Git(String),

    /// Raised when a worktree cannot be found.
    #[error("{0}")]
    WorktreeNotFound(String),

    /// Raised when a branch is invalid or in an unexpected state.
    #[error("{0}")]
    InvalidBranch(String),

    /// Raised when a merge operation fails.
    #[error("{0}")]
    Merge(String),

    /// Raised when a rebase operation fails.
    #[error("{0}")]
    Rebase(String),

    /// Raised when a hook execution fails.
    #[error("{0}")]
    Hook(String),

    /// Raised when configuration operations fail.
    #[error("{0}")]
    Config(String),

    /// I/O errors.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization errors.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CwError>;
