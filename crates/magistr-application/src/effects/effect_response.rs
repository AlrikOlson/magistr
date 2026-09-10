use super::TerminalExit;
use alloc::string::String;

/// Typed result from the configured host capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectResponse {
    Text(String),
    Written,
    TerminalCreated(String),
    TerminalOutput {
        output: String,
        truncated: bool,
        exit: Option<TerminalExit>,
    },
    TerminalExited(TerminalExit),
    TerminalKilled,
    TerminalReleased,
}
