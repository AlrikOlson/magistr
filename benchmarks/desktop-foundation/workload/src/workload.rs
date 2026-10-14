//! The comparison corpus and ordered stream contract change together: neither
//! adapter may reduce retained content or silently lose a producer's update.
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Fixture {
    pub items: Vec<String>,
    pub messages: Vec<String>,
    pub diff: Vec<String>,
}
impl Fixture {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: (0..1_000).map(|i| format!("{} · {} · {}", i + 1,
                ["Agent workspace", "Source review", "Planning evidence", "Project setup"][i % 4],
                ["Ready", "Working", "Waiting for you", "Complete"][i % 4])).collect(),
            messages: (0..160).map(|i| format!(
                "Review {}: workspace ownership\nThe project and its working copy keep one owner. Résumé · 東京 · مرحبا · Ελληνικά. Read the recorded evidence before starting the next item.\nlet workspace = project.open()?;\nRequirements\nKeep sessions running when another workspace is selected.\nPreserve the exact source and the user's unsent request.\nEvidence | Result\nOwner admission | Passed\nhttps://example.org/evidence/{}", i + 1, i + 1)).collect(),
            diff: (0..8_000).map(|i| format!("{}{:05}  {}",
                if i % 3 == 0 { "+" } else if i % 3 == 1 { "-" } else { " " },
                i + 1, if i % 3 == 0 { "session.observe(workspace.identity());" } else {
                    "let evidence = owner.current_snapshot()?;"
                })).collect(),
        }
    }
}
impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub stream: usize,
    pub sequence: u64,
    pub sent_us: u64,
    pub text: String,
}

/// A shared clock for transport-to-render acknowledgement across processes.
#[must_use]
pub fn timestamp_us() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("benchmark clock is after the epoch")
            .as_micros(),
    )
    .unwrap_or(u64::MAX)
}

/// Cancellation belongs to the producer, including an interruptible wait.
#[derive(Clone)]
pub struct StreamCancellation {
    stopped: Arc<AtomicBool>,
    threads: Vec<thread::Thread>,
}
impl StreamCancellation {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        for producer in &self.threads {
            producer.unpark();
        }
    }
}

/// Eight independent, bounded producers; adapters must account for every event.
pub struct StreamRun {
    pub receiver: mpsc::Receiver<Event>,
    pub started: Instant,
    dropped: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}
impl StreamRun {
    #[must_use]
    pub fn start(count: u64, period: Duration) -> Self {
        let (sender, receiver) = mpsc::sync_channel(4_096);
        let dropped = Arc::new(AtomicU64::new(0));
        let stopped = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let threads = (0..8)
            .map(|stream| {
                let sender = sender.clone();
                let dropped = Arc::clone(&dropped);
                let stopped = Arc::clone(&stopped);
                thread::spawn(move || {
                    for sequence in 0..count {
                        let elapsed = period.mul_f64(sequence as f64);
                        let due = started + elapsed;
                        while !stopped.load(Ordering::Acquire) {
                            let Some(wait) = due.checked_duration_since(Instant::now()) else {
                                break;
                            };
                            thread::park_timeout(wait);
                        }
                        if stopped.load(Ordering::Acquire) {
                            break;
                        }
                        let event = Event {
                            stream,
                            sequence,
                            sent_us: timestamp_us(),
                            text: format!(
                                "Agent {} update {:04}: source reviewed · 東京 · résumé.\n",
                                stream + 1,
                                sequence
                            ),
                        };
                        match sender.try_send(event) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                dropped.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                })
            })
            .collect();
        drop(sender);
        Self {
            receiver,
            started,
            dropped,
            stopped,
            threads,
        }
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn cancellation(&self) -> StreamCancellation {
        StreamCancellation {
            stopped: Arc::clone(&self.stopped),
            threads: self
                .threads
                .iter()
                .map(|producer| producer.thread().clone())
                .collect(),
        }
    }
}
impl Drop for StreamRun {
    fn drop(&mut self) {
        self.cancellation().stop();
        for producer in self.threads.drain(..) {
            let _ = producer.join();
        }
    }
}

#[derive(Default)]
pub struct StreamState {
    pub sequences: [u64; 8],
    pub texts: [String; 8],
}
impl StreamState {
    /// Refuse an invalid source or sequence without changing retained content.
    pub fn apply(&mut self, event: &Event) -> Result<(), String> {
        let expected = self
            .sequences
            .get_mut(event.stream)
            .ok_or_else(|| format!("Unknown stream {}", event.stream))?;
        if event.sequence != *expected {
            return Err(format!(
                "Stream {} expected {}, received {}",
                event.stream, expected, event.sequence
            ));
        }
        self.texts[event.stream].push_str(&event.text);
        *expected += 1;
        Ok(())
    }
}

#[cfg(test)]
#[path = "workload.tests.rs"]
mod tests;
