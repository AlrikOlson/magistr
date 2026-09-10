use alloc::{string::String, vec::Vec};

/// Protocol-independent host request. Paths are UTF-8; adapters reject lossy
/// conversion. Terminal lifecycle IDs are scoped to their creating session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectRequest {
    ReadTextFile {
        session_id: String,
        path: String,
        line: Option<u32>,
        limit: Option<u32>,
    },
    WriteTextFile {
        session_id: String,
        path: String,
        content: String,
    },
    CreateTerminal {
        session_id: String,
        command: String,
        executable: Option<String>,
        args: Vec<String>,
        env: Vec<(String, String)>,
        cwd: Option<String>,
        output_byte_limit: Option<u64>,
    },
    TerminalOutput {
        session_id: String,
        terminal_id: String,
    },
    WaitForTerminalExit {
        session_id: String,
        terminal_id: String,
    },
    KillTerminal {
        session_id: String,
        terminal_id: String,
    },
    ReleaseTerminal {
        session_id: String,
        terminal_id: String,
    },
}

impl EffectRequest {
    #[must_use]
    pub fn session_id(&self) -> &str {
        match self {
            Self::ReadTextFile { session_id, .. }
            | Self::WriteTextFile { session_id, .. }
            | Self::CreateTerminal { session_id, .. }
            | Self::TerminalOutput { session_id, .. }
            | Self::WaitForTerminalExit { session_id, .. }
            | Self::KillTerminal { session_id, .. }
            | Self::ReleaseTerminal { session_id, .. } => session_id,
        }
    }

    /// Cleanup remains possible after a turn has been cancelled.
    #[must_use]
    pub fn is_cleanup(&self) -> bool {
        matches!(
            self,
            Self::KillTerminal { .. } | Self::ReleaseTerminal { .. }
        )
    }
}
