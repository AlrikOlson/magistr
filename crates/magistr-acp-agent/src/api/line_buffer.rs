//! Reassemble lines from a byte stream that splits them anywhere.

use crate::AgentError;

/// Bytes arrive in whatever pieces the socket delivers; the APIs speak in
/// lines (SSE fields, NDJSON objects). Push each piece, drain the complete
/// lines, and finish with whatever the stream ended on.
#[derive(Debug, Default)]
pub(crate) struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    /// Append a piece of the body.
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
    }

    /// Every complete line so far, without its `\n` (and `\r`), in order.
    /// Fails on a line that is not UTF-8.
    pub(crate) fn drain_lines(&mut self) -> Result<Vec<String>, AgentError> {
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = self.pending.drain(..=end).collect();
            lines.push(decode(&raw[..end])?);
        }
        Ok(lines)
    }

    /// The unterminated last line, if the body ended without a newline.
    pub(crate) fn finish(&mut self) -> Result<Option<String>, AgentError> {
        if self.pending.is_empty() {
            return Ok(None);
        }
        let raw = std::mem::take(&mut self.pending);
        decode(&raw).map(Some)
    }
}

fn decode(raw: &[u8]) -> Result<String, AgentError> {
    let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|e| AgentError::provider(format!("the response is not UTF-8: {e}")))
}

#[cfg(test)]
#[path = "line_buffer.tests.rs"]
mod tests;
