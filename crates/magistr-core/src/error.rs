//! Core error types for Magistr.
//!
//! Defines `MagistrError` and `Result<T>` used throughout the codebase.
//! This module is frontend-agnostic — no `color_eyre`, no `acp_sdk`.
//! CLI-specific extensions (eyre integration, `From<ClaudeError>`) live in
//! `magistr-cli/src/core/error.rs`.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MagistrError {
    #[error("IO error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("detection failed: {0}")]
    Detection(String),

    #[error("preset not found for language: {0}")]
    PresetNotFound(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("template error: {0}")]
    Template(String),

    #[error("directory not found: {0}")]
    DirNotFound(PathBuf),

    #[error("budget exceeded: {0}")]
    Budget(String),

    #[error("sqlite error: {context}: {cause}")]
    Sqlite { context: String, cause: String },

    #[error("parse error ({format}): {context}: {cause}")]
    Parse {
        format: String,
        context: String,
        cause: String,
    },

    #[error("Agent error: {0}")]
    Acp(String),

    #[error("sync lock error: {0}")]
    SyncLock(String),

    #[error("task join error: {0}")]
    TaskJoin(String),

    #[error("cancelled: {0}")]
    Cancelled(String),

    #[error("plugin error: {0}")]
    Plugin(String),

    #[error("workflow error: {0}")]
    Workflow(String),

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error("engine error: {0}")]
    Engine(String),

    #[error("model error: {0}")]
    Model(String),

    #[error("permission error: {0}")]
    Permission(String),

    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for MagistrError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialize(err.to_string())
    }
}

impl From<std::fmt::Error> for MagistrError {
    fn from(err: std::fmt::Error) -> Self {
        Self::Serialize(err.to_string())
    }
}

impl MagistrError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Create a SQLite error with operation context.
    pub fn sqlite(context: impl Into<String>, cause: impl std::fmt::Display) -> Self {
        Self::Sqlite {
            context: context.into(),
            cause: cause.to_string(),
        }
    }

    /// Create a parse error with format and context.
    pub fn parse(
        format: impl Into<String>,
        context: impl Into<String>,
        cause: impl std::fmt::Display,
    ) -> Self {
        Self::Parse {
            format: format.into(),
            context: context.into(),
            cause: cause.to_string(),
        }
    }

    /// Returns an actionable suggestion for how to fix this error, if applicable.
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                Some("Check that the path exists and you have read permissions.")
            }
            Self::Io { source, .. } if source.kind() == std::io::ErrorKind::PermissionDenied => {
                Some("Check file permissions, or try running with appropriate access.")
            }
            Self::Io { .. } => None,
            Self::Detection(_) => Some(
                "Ensure you're running from a project directory with a manifest file\n  (Cargo.toml, package.json, pyproject.toml, go.mod, etc.).",
            ),
            Self::PresetNotFound(_) => Some(
                "Run `magistr detect` to check what language was detected.\n  If incorrect, ensure the project has a valid manifest file.",
            ),
            Self::Config(_) => {
                Some("Correct the configuration reported above, then run the command again.")
            }
            Self::Template(_) => None,
            Self::DirNotFound(_) => Some("Verify the directory path is correct and exists."),
            Self::Budget(_) => {
                Some("Increase the budget with `--budget <amount>` or remove the limit.")
            }
            Self::Sqlite { .. } => Some(
                "Check that the database file is not locked or corrupted.\n  Try removing ~/.local/share/magistr/sessions.db to reset.",
            ),
            Self::Parse { .. } => Some("Check the file for syntax errors."),
            Self::Acp(msg) => {
                let lower = msg.to_lowercase();
                let is_auth = lower.contains("authentication required")
                    || lower.contains("no authentication configured")
                    || lower.contains("unauthorized")
                    || lower.contains("invalid api key")
                    || lower.contains("invalid x-api-key");
                // The binary itself is missing: the only case an install hint fits.
                let is_missing_binary = lower.contains("binary not found")
                    || lower.contains("no acp-compatible agent")
                    || lower.contains("not found on path")
                    || lower.contains("no such file");
                // The agent started and refused, or died, during the handshake:
                // the message already quotes its own reason (and stderr tail).
                let is_handshake = lower.contains("initialize failed")
                    || lower.contains("session/new failed")
                    || lower.contains("session/load failed")
                    || lower.contains("agent exited during")
                    || lower.contains("handshake")
                    || lower.contains("failed to spawn acp agent");
                if is_auth {
                    None
                } else if is_missing_binary {
                    Some(
                        "Check that Claude Code (the `claude` command) is installed and on PATH.\n  \
                         Install with: curl -fsSL https://claude.ai/install.sh | bash",
                    )
                } else if is_handshake {
                    Some(
                        "The agent is installed but did not accept a session — the reason above \
                         (and any `agent stderr` lines) is the agent's own.\n  \
                         claude reads ~/.claude/settings.json itself: fix the setting it \
                         names and retry. Pointing CLAUDE_CONFIG_DIR elsewhere is not a workaround — \
                         it also hides the keychain login.",
                    )
                } else {
                    None
                }
            }
            Self::SyncLock(_) => Some(
                "Another sync operation may be in progress.\n  \
                 Check for stale lock files at .magistr/roadmap.lock and remove if needed.",
            ),
            Self::TaskJoin(_) => None,
            Self::Cancelled(_) => None,
            Self::Plugin(_) => {
                Some("Check plugin manifest and installation with `magistr plugins validate`.")
            }
            Self::Workflow(_) => {
                Some("Check that .magistr/ directory exists. Run `magistr init` to set up.")
            }
            Self::Serialize(_) => {
                Some("Check the output path is writable and the data format is valid.")
            }
            Self::Engine(_) => Some("Check that the agent backend is running and responsive."),
            Self::Model(_) => Some("Run `magistr models` to see available models."),
            Self::Permission(_) => {
                Some("Check permissions audit log at ~/.local/share/magistr/permissions.log.")
            }
            Self::Other(_) => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, MagistrError>;

/// Extension trait for converting any `Result<T, E: Display>` into `Result<T, MagistrError>`
/// using a variant constructor.
///
/// Replaces the mechanical `.map_err(|e| MagistrError::X(e.to_string()))` pattern.
///
/// # Usage
/// ```ignore
/// some_result.with_context(MagistrError::Plugin)?;
/// ```
pub trait MapMagistrErr<T, E> {
    fn with_context(self, f: impl FnOnce(String) -> MagistrError) -> Result<T>;
}

impl<T, E: std::fmt::Display> MapMagistrErr<T, E> for std::result::Result<T, E> {
    fn with_context(self, f: impl FnOnce(String) -> MagistrError) -> Result<T> {
        self.map_err(|e| f(e.to_string()))
    }
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
