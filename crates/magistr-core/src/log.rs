//! Log message types — severity-tagged messages for display.

/// Severity level for log messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// A log message with a severity level.
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub level: LogLevel,
    pub text: String,
}

#[cfg(test)]
#[path = "log.tests.rs"]
mod tests;
