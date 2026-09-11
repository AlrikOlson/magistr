/// A stream capture that keeps the first three quarters and the last quarter
/// of its ceiling and counts everything in between. A runaway producer costs
/// the ceiling, never the host's memory; the retained head+tail cut still
/// sees both ends of the transcript.
#[derive(Debug, Default)]
pub(super) struct BoundedCapture {
    head_limit: usize,
    tail_limit: usize,
    head: Vec<u8>,
    tail: Vec<u8>,
    dropped: u64,
}

impl BoundedCapture {
    pub(super) fn new(capture_bytes: usize) -> Self {
        let capture_bytes = capture_bytes.max(8);
        let tail_limit = capture_bytes / 4;
        Self {
            head_limit: capture_bytes - tail_limit,
            tail_limit,
            ..Self::default()
        }
    }

    pub(super) fn push(&mut self, bytes: &[u8]) {
        let head_room = self.head_limit.saturating_sub(self.head.len());
        let (to_head, rest) = bytes.split_at(head_room.min(bytes.len()));
        self.head.extend_from_slice(to_head);
        if rest.is_empty() {
            return;
        }
        if rest.len() >= self.tail_limit {
            self.dropped += (self.tail.len() + rest.len() - self.tail_limit) as u64;
            self.tail.clear();
            self.tail
                .extend_from_slice(&rest[rest.len() - self.tail_limit..]);
            return;
        }
        self.tail.extend_from_slice(rest);
        if self.tail.len() > self.tail_limit {
            let excess = self.tail.len() - self.tail_limit;
            self.tail.drain(..excess);
            self.dropped += excess as u64;
        }
    }

    pub(super) fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The transcript as text. When bytes were discarded, one marker line
    /// names the gap; no parser regex matches it, so structure extraction is
    /// unaffected and the cut is visible rather than silent.
    pub(super) fn text(&self) -> String {
        let mut text = String::from_utf8_lossy(&self.head).into_owned();
        if self.dropped > 0 {
            text.push_str(&format!(
                "\n[magistr run: {} bytes of output dropped between head and tail]\n",
                self.dropped
            ));
        }
        text.push_str(&String::from_utf8_lossy(&self.tail));
        text
    }
}

#[cfg(test)]
#[path = "bounded_capture.tests.rs"]
mod tests;
