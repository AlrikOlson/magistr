//! The agent's last stderr lines, kept for error reports.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::AcpError;

/// Number of trailing stderr lines kept.
const STDERR_TAIL_LINES: usize = 20;

/// The last few lines of the agent's stderr — the only place the agent's own
/// reason survives when it dies before answering a request. Written by the
/// stderr drain task, read wherever an error is composed.
#[derive(Clone, Default)]
pub(crate) struct StderrTail(Arc<Mutex<VecDeque<String>>>);

impl StderrTail {
    pub(crate) fn push(&self, line: String) {
        if let Ok(mut lines) = self.0.lock() {
            if lines.len() >= STDERR_TAIL_LINES {
                lines.pop_front();
            }
            lines.push_back(line);
        }
    }

    /// Append the captured lines to `message` (unchanged when nothing was captured).
    pub(crate) fn append_to(&self, message: String) -> String {
        let lines: Vec<String> = self
            .0
            .lock()
            .map(|lines| lines.iter().cloned().collect())
            .unwrap_or_default();
        if lines.is_empty() {
            message
        } else {
            format!("{message}\n  agent stderr:\n    {}", lines.join("\n    "))
        }
    }

    /// Re-wrap `err` with the captured stderr lines appended to its message.
    pub(crate) fn attach(&self, err: AcpError) -> AcpError {
        match err {
            AcpError::Connection(m) => AcpError::Connection(self.append_to(m)),
            AcpError::Transport(m) => AcpError::Transport(self.append_to(m)),
            AcpError::Timeout(m) => AcpError::Timeout(self.append_to(m)),
            other => other,
        }
    }
}

#[cfg(test)]
#[path = "stderr_tail.tests.rs"]
mod tests;
