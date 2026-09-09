//! Agent backend selection and capability metadata.
//!
//! Defines which agent backend to use for a session and what features
//! each backend supports. These types are frontend-agnostic — both the
//! TUI and GUI use them to display backend info and capabilities.

/// Which agent backend to use for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// Claude Code via ACP (Agent Client Protocol) — formal lifecycle, graceful cancel.
    #[default]
    Acp,
}

impl Backend {
    /// Get the capabilities of this backend.
    #[must_use]
    pub fn capabilities(self) -> BackendCapabilities {
        match self {
            Self::Acp => BackendCapabilities {
                name: "Claude Code (ACP)",
                streaming: true,
                session_resume: true,
                model_selection: true,
                mcp_support: true,
                permission_profiles: true,
                graceful_cancel: true,
                session_state: true,
                streaming_notifications: true,
            },
        }
    }

    /// Whether this backend supports graceful cancel (ACP `session/cancel`).
    #[must_use]
    pub fn supports_cancel(self) -> bool {
        matches!(self, Self::Acp)
    }

    /// Human-readable label for this backend.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Acp => "acp",
        }
    }

    /// All backends that are compiled in (regardless of availability).
    #[must_use]
    pub fn all_compiled() -> Vec<Self> {
        vec![Self::Acp]
    }
}

/// Feature capabilities of an agent backend.
///
/// Used by `magistr models` to show which features each backend supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendCapabilities {
    /// Human-readable backend name.
    pub name: &'static str,
    /// Whether the backend streams partial text as it arrives.
    pub streaming: bool,
    /// Whether sessions can be resumed with `--continue`.
    pub session_resume: bool,
    /// Whether the `--model` flag is supported.
    pub model_selection: bool,
    /// Whether MCP servers are forwarded to the backend.
    pub mcp_support: bool,
    /// Whether magistr's `--trust` permission profiles are mapped.
    pub permission_profiles: bool,
    /// Whether the backend supports graceful cancel (stop after current tool).
    pub graceful_cancel: bool,
    /// Whether the backend exposes formal session lifecycle state.
    pub session_state: bool,
    /// Whether the backend uses streaming notifications (vs polling).
    pub streaming_notifications: bool,
}

impl BackendCapabilities {
    /// Features as a list of (name, supported) pairs for display.
    #[must_use]
    pub fn feature_list(&self) -> Vec<(&'static str, bool)> {
        vec![
            ("Streaming output", self.streaming),
            ("Session resume", self.session_resume),
            ("Model selection", self.model_selection),
            ("MCP support", self.mcp_support),
            ("Permission profiles", self.permission_profiles),
            ("Graceful cancel", self.graceful_cancel),
            ("Session state", self.session_state),
            ("Streaming notifications", self.streaming_notifications),
        ]
    }
}

#[cfg(test)]
#[path = "backend.tests.rs"]
mod tests;
