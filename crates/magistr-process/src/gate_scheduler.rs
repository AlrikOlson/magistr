//! Shared admission for verification processes, independent of their transport.
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Resources and capacity for one verification command.
#[derive(Clone, Debug)]
pub struct GateSchedule {
    /// Maximum concurrent commands for this workspace, capped at sixteen.
    pub max_parallel: usize,
    /// None is inferred; an empty list explicitly declares an independent check.
    pub resources: Option<Vec<String>>,
}
impl Default for GateSchedule {
    fn default() -> Self {
        Self {
            max_parallel: 4,
            resources: None,
        }
    }
}
#[derive(Clone)]
struct Request {
    id: u64,
    root: PathBuf,
    resources: Option<Vec<String>>,
    limit: usize,
}
#[derive(Default)]
struct State {
    next: u64,
    active: Vec<Request>,
    waiting: VecDeque<Request>,
}
#[derive(Default)]
struct Queue {
    state: Mutex<State>,
    changed: Notify,
}
/// A held execution slot. Dropping it releases every resource atomically.
pub struct GatePermit {
    queue: Arc<Queue>,
    id: u64,
}
impl Drop for GatePermit {
    fn drop(&mut self) {
        let mut state = self.queue.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active.retain(|request| request.id != self.id);
        state.waiting.retain(|request| request.id != self.id);
        drop(state);
        self.queue.changed.notify_waiters();
    }
}
/// Why a queued command did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateWaitError {
    /// The caller cancelled before execution.
    Cancelled,
    /// The total execution deadline expired in the queue.
    TimedOut,
}

/// Wait for capacity and resources. One process-wide queue is shared by pipes,
/// PTYs, MCP requests and loop jobs; no Cargo subprocess waits on another gate's lock.
pub async fn acquire_gate(
    root: &Path,
    command: &str,
    schedule: &GateSchedule,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<GatePermit, GateWaitError> {
    static QUEUE: OnceLock<Arc<Queue>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| Arc::new(Queue::default())).clone();
    let root = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    let resources = resources(&root, command, schedule.resources.as_deref());
    let request = {
        let mut state = queue.state.lock().unwrap_or_else(|e| e.into_inner());
        state.next += 1;
        let request = Request {
            id: state.next,
            root,
            resources,
            limit: schedule.max_parallel.clamp(1, 16),
        };
        state.waiting.push_back(request.clone());
        request
    };
    let permit = GatePermit {
        queue: queue.clone(),
        id: request.id,
    };
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        // Register before examining state: a release cannot be lost between
        // the readiness check and awaiting the notification.
        let notified = queue.changed.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if cancel.is_cancelled() {
            return Err(GateWaitError::Cancelled);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(GateWaitError::TimedOut);
        }
        {
            let mut state = queue.state.lock().unwrap_or_else(|e| e.into_inner());
            let capacity = state
                .active
                .iter()
                .filter(|active| active.root == request.root)
                .count()
                < request.limit;
            let conflict = state
                .active
                .iter()
                .any(|active| conflicts(active, &request));
            let earlier = state
                .waiting
                .iter()
                .take_while(|waiting| waiting.id != request.id)
                .any(|waiting| conflicts(waiting, &request));
            if capacity && !conflict && !earlier {
                state.waiting.retain(|waiting| waiting.id != request.id);
                state.active.push(request);
                return Ok(permit);
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => return Err(GateWaitError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => return Err(GateWaitError::TimedOut),
            _ = &mut notified => {}
        }
    }
}
fn conflicts(a: &Request, b: &Request) -> bool {
    match (&a.resources, &b.resources) {
        (Some(a), Some(b)) => a.iter().any(|resource| b.contains(resource)),
        _ => a.root == b.root,
    }
}
fn resources(root: &Path, command: &str, declared: Option<&[String]>) -> Option<Vec<String>> {
    if let Some(resources) = declared {
        return Some(
            resources
                .iter()
                .map(|resource| {
                    if resource == "cargo" {
                        cargo_resource(root)
                    } else {
                        format!("{}:{resource}", root.display())
                    }
                })
                .collect(),
        );
    }
    let commands: Vec<_> = command.split("&&").map(str::trim).collect();
    if commands.iter().all(|part| part.starts_with("cargo ")) {
        return Some(vec![cargo_resource(root)]);
    }
    // Unknown scripts may invoke Cargo, npm or mutate build outputs.
    None
}

fn cargo_resource(root: &Path) -> String {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let target = if target.is_absolute() {
        target
    } else {
        root.join(target)
    };
    let target = target.canonicalize().unwrap_or(target);
    format!("cargo:{}", target.display())
}

#[cfg(test)]
#[path = "gate_scheduler.tests.rs"]
mod tests;
