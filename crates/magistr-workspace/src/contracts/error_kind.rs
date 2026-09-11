use super::Recovery;

/// Each failure fixes its recovery class and requires its actionable payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    PathOutsideWorkspace,
    ProtectedPath,
    AmbiguousSymbol {
        candidates: Vec<String>,
    },
    SymbolNotFound,
    FileNotFound,
    ParseRejected {
        parser_message: String,
        rejected_diff: String,
    },
    StaleAnchor {
        content_hash: String,
    },
    PatternInvalid,
    BudgetTooSmall,
    CommandNotDeclared,
    RunRedirected,
    SandboxFuelExceeded,
    SandboxCapDenied,
    SandboxMemoryExceeded,
    IndexUnavailable,
    ConfirmRequired {
        confirm_token: String,
    },
    ConfirmMismatch,
    UnsupportedVersion,
    EngineUnavailable,
    EngineBusy {
        backoff_ms: u64,
    },
    InputQuarantined,
    PluginCapDenied,
    PluginCrashed {
        plugin: String,
    },
    ConfigPolicyDenied,
}

impl ErrorKind {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PathOutsideWorkspace => "PATH_OUTSIDE_WORKSPACE",
            Self::ProtectedPath => "PROTECTED_PATH",
            Self::AmbiguousSymbol { .. } => "AMBIGUOUS_SYMBOL",
            Self::SymbolNotFound => "SYMBOL_NOT_FOUND",
            Self::FileNotFound => "FILE_NOT_FOUND",
            Self::ParseRejected { .. } => "PARSE_REJECTED",
            Self::StaleAnchor { .. } => "STALE_ANCHOR",
            Self::PatternInvalid => "PATTERN_INVALID",
            Self::BudgetTooSmall => "BUDGET_TOO_SMALL",
            Self::CommandNotDeclared => "COMMAND_NOT_DECLARED",
            Self::RunRedirected => "RUN_REDIRECTED",
            Self::SandboxFuelExceeded => "SANDBOX_FUEL_EXCEEDED",
            Self::SandboxCapDenied => "SANDBOX_CAP_DENIED",
            Self::SandboxMemoryExceeded => "SANDBOX_MEMORY_EXCEEDED",
            Self::IndexUnavailable => "INDEX_UNAVAILABLE",
            Self::ConfirmRequired { .. } => "CONFIRM_REQUIRED",
            Self::ConfirmMismatch => "CONFIRM_MISMATCH",
            Self::UnsupportedVersion => "UNSUPPORTED_VERSION",
            Self::EngineUnavailable => "ENGINE_UNAVAILABLE",
            Self::EngineBusy { .. } => "ENGINE_BUSY",
            Self::InputQuarantined => "INPUT_QUARANTINED",
            Self::PluginCapDenied => "PLUGIN_CAP_DENIED",
            Self::PluginCrashed { .. } => "PLUGIN_CRASHED",
            Self::ConfigPolicyDenied => "CONFIG_POLICY_DENIED",
        }
    }

    #[must_use]
    pub const fn recovery(&self) -> Recovery {
        match self {
            Self::PathOutsideWorkspace
            | Self::CommandNotDeclared
            | Self::RunRedirected
            | Self::UnsupportedVersion
            | Self::PluginCrashed { .. }
            | Self::ConfigPolicyDenied => Recovery::Abort,
            Self::ProtectedPath
            | Self::SandboxCapDenied
            | Self::InputQuarantined
            | Self::PluginCapDenied => Recovery::Escalate,
            Self::AmbiguousSymbol { .. }
            | Self::SymbolNotFound
            | Self::FileNotFound
            | Self::ParseRejected { .. }
            | Self::PatternInvalid
            | Self::BudgetTooSmall
            | Self::SandboxFuelExceeded
            | Self::SandboxMemoryExceeded => Recovery::Narrow,
            Self::StaleAnchor { .. }
            | Self::IndexUnavailable
            | Self::EngineUnavailable
            | Self::EngineBusy { .. } => Recovery::Retry,
            Self::ConfirmRequired { .. } | Self::ConfirmMismatch => Recovery::Confirm,
        }
    }
}
