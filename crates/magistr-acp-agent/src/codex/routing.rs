//! The single synchronized routing table, including connection failure.

use super::frame::Frame;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) struct Routing {
    pub pending: HashMap<u64, oneshot::Sender<Result<Value, String>>>,
    pub threads: HashMap<String, mpsc::UnboundedSender<Result<Frame, String>>>,
    pub active: HashMap<String, Option<String>>,
    pub starting_requests: HashMap<u64, String>,
    pub failure: Option<String>,
    pub failure_signal: watch::Sender<Option<String>>,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
            threads: HashMap::new(),
            active: HashMap::new(),
            starting_requests: HashMap::new(),
            failure: None,
            failure_signal: watch::channel(None).0,
        }
    }
}

impl Routing {
    /// Fail every waiter and make later registration fail atomically.
    pub fn fail(&mut self, error: String) {
        if self.failure.is_some() {
            return;
        }
        self.failure = Some(error.clone());
        self.failure_signal.send_replace(Some(error.clone()));
        for (_, pending) in self.pending.drain() {
            let _ = pending.send(Err(error.clone()));
        }
        for (_, thread) in self.threads.drain() {
            let _ = thread.send(Err(error.clone()));
        }
    }
}
