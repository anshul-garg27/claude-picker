//! Typed errors for the claude-picker data layer.
//!
//! Uses `thiserror` for structured variants and re-exports a crate-wide
//! [`Result`] alias. Most public functions that can fail either return this
//! [`Result`] or the broader `anyhow::Result` when a call site needs to
//! attach extra context.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Claude CLI not available on PATH")]
    ClaudeCliMissing,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
