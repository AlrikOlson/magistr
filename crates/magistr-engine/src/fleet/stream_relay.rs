use std::collections::BTreeMap;

use crate::error::EngineError;
use crate::session::SessionEvent;

/// The ordered stream of one job's session events, as the control plane
/// holds it between the node producing them and the client reading them.
///
/// Two clocks meet here and only one of them is the reader's.
///
/// The **node's** clock is a per-attempt sequence number. A node numbers
/// the events of the attempt it currently holds from zero; the relay
/// buffers a gap, drops a repeat below the watermark, and drains
/// contiguously — so a transport that delivers at-least-once and out of
/// order becomes exactly-once and in order by the time anything reads it.
///
/// The **reader's** clock is a cursor over [`delivered`], which is
/// append-only *across attempts*. [`restart`] resets the node's sequence
/// and throws away whatever was still buffered, but never truncates what
/// has already been handed out. So a job reclaimed from a dead node and
/// re-run somewhere else appends to what the reader already saw rather
/// than rewinding it: the reader may see a turn twice, which is what
/// at-least-once execution means, but never sees its cursor move
/// backwards.
///
/// The attempt number is the fence between the two. An event stamped
/// with an attempt the relay has moved past belongs to a node that no
/// longer holds this job, and is refused rather than interleaved — the
/// same token [`WorkQueue::complete`] uses to refuse a zombie's late
/// success.
///
/// [`delivered`]: StreamRelay::delivered
/// [`restart`]: StreamRelay::restart
/// [`WorkQueue::complete`]: super::work_queue::WorkQueue::complete
#[derive(Debug, Clone, Default)]
pub struct StreamRelay {
    attempt: u32,
    next_seq: u64,
    pending: BTreeMap<u64, SessionEvent>,
    delivered: Vec<SessionEvent>,
}

impl StreamRelay {
    /// How many out-of-order events may wait for the gap ahead of them.
    ///
    /// A node that skips a sequence number and never sends it would
    /// otherwise buffer without bound. Past the cap the relay refuses
    /// the event, which surfaces the node's bug at the node instead of
    /// growing the control plane's memory quietly. The event the relay
    /// is actually waiting on is exempt — it drains rather than waits,
    /// and refusing it would leave a full buffer with no way to empty.
    pub const MAX_PENDING: usize = 256;

    /// An empty relay, waiting for attempt 0's first event.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The attempt this relay is currently accepting events for.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// The next sequence number the relay is waiting on.
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Everything handed out so far, oldest first.
    #[must_use]
    pub fn delivered(&self) -> &[SessionEvent] {
        &self.delivered
    }

    /// Everything after `cursor`, so a client can resume where it left
    /// off. A cursor past the end reads as "nothing new" rather than an
    /// error — a reader that is ahead of us is a reader with nothing to
    /// do, not a bad request.
    #[must_use]
    pub fn since(&self, cursor: usize) -> &[SessionEvent] {
        self.delivered.get(cursor..).unwrap_or(&[])
    }

    /// How many events are buffered waiting for a gap to close.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.pending.len()
    }

    /// Take an event a node produced for `attempt`, numbered `seq`.
    ///
    /// Returns how many events became deliverable — zero for a repeat,
    /// zero for one that lands past a gap, and more than one when the
    /// event that arrived was the gap itself.
    ///
    /// Refused when `attempt` is not the one this relay is on, or when
    /// the buffer is full.
    pub fn accept(
        &mut self,
        attempt: u32,
        seq: u64,
        event: SessionEvent,
    ) -> Result<usize, EngineError> {
        if attempt != self.attempt {
            return Err(EngineError::Precondition(format!(
                "event for attempt {attempt} rejected: the relay is on attempt {}",
                self.attempt
            )));
        }
        if seq < self.next_seq || self.pending.contains_key(&seq) {
            return Ok(0);
        }
        // The cap is on what *waits*, so the event that closes the gap is
        // always taken: refusing it would wedge a full buffer for ever.
        if seq > self.next_seq && self.pending.len() >= Self::MAX_PENDING {
            return Err(EngineError::Precondition(format!(
                "event {seq} rejected: {} events are already waiting on sequence {}",
                self.pending.len(),
                self.next_seq
            )));
        }
        self.pending.insert(seq, event);
        Ok(self.drain())
    }

    /// Move the relay onto a new attempt: the node's sequence starts
    /// over, whatever was still waiting on the old attempt's gaps is
    /// dropped, and the delivered log is left exactly as it is.
    pub fn restart(&mut self, attempt: u32) {
        self.attempt = attempt;
        self.next_seq = 0;
        self.pending.clear();
    }

    /// Hand out every buffered event that is now contiguous.
    fn drain(&mut self) -> usize {
        let mut moved = 0;
        while let Some(event) = self.pending.remove(&self.next_seq) {
            self.delivered.push(event);
            self.next_seq += 1;
            moved += 1;
        }
        moved
    }
}

#[cfg(test)]
#[path = "stream_relay.tests.rs"]
mod tests;
