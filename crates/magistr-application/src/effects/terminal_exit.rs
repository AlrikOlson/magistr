use alloc::string::String;

/// Observed process termination, independent of a terminal protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExit {
    pub exit_code: Option<u32>,
    pub signal: Option<String>,
}
