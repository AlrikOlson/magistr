//! Agent pool — lifecycle management for multiple concurrent ACP sessions.
//!
//! `AgentPool` is the central coordinator that spawns, monitors, and cancels
//! worker agent sessions. It multiplexes messages from all workers into a
//! single stream for the supervisor to consume.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::sync::mpsc;

use super::role::SpecializedRole;
use super::tool_policy::apply_tool_policy;
use super::topology::{AgentId, AgentRole, AgentTopology, WorkerStatus};
use crate::agent::AgentConfig;
use crate::agent::message::AgentMessage;
use crate::agent::session::AgentSession;
use crate::error::Result;

// ─── Pool Event ────────────────────────────────────────────────────────────

/// An event from a worker in the pool, tagged with the worker's ID.
#[derive(Debug)]
pub struct PoolEvent {
    /// Which agent produced this event.
    pub agent_id: AgentId,
    /// The agent message.
    pub message: Result<AgentMessage>,
}

// ─── Worker Handle ─────────────────────────────────────────────────────────

/// Handle to a running worker agent session.
///
/// The actual `AgentSession` is moved into a reader task. This handle retains
/// a cancel channel for lifecycle control and tracks status/cost metadata.
pub struct WorkerHandle {
    /// Channel to signal cancellation to the reader task.
    cancel_tx: mpsc::UnboundedSender<()>,
    /// Worker's role in the topology.
    pub role: AgentRole,
    /// Specialized role ID (e.g., "researcher", "coder").
    pub specialized_role: Option<String>,
    /// Current lifecycle status.
    pub status: WorkerStatus,
    /// Task description assigned to this worker.
    pub task_description: String,
    /// Files this worker is scoped to.
    pub scope: Vec<PathBuf>,
    /// Cumulative cost in USD for this worker.
    pub cost_usd: f64,
    /// Number of tokens consumed by this worker.
    pub tokens_used: u64,
}

impl std::fmt::Debug for WorkerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerHandle")
            .field("role", &self.role)
            .field("specialized_role", &self.specialized_role)
            .field("status", &self.status)
            .field("task_description", &self.task_description)
            .field("scope", &self.scope)
            .field("cost_usd", &self.cost_usd)
            .field("tokens_used", &self.tokens_used)
            .finish_non_exhaustive()
    }
}

// ─── Pool ──────────────────────────────────────────────────────────────────

/// Manages the lifecycle of multiple concurrent agent sessions.
///
/// Workers are spawned with `spawn_worker()` and produce events via `next_event()`.
/// The pool multiplexes all worker message streams into a single output channel.
pub struct AgentPool {
    /// Active workers keyed by their agent ID.
    workers: HashMap<AgentId, WorkerHandle>,
    /// The agent topology describing roles and connections.
    topology: AgentTopology,
    /// Counter for generating unique agent IDs.
    next_id: AtomicU32,
    /// Multiplexed event receiver.
    event_rx: mpsc::UnboundedReceiver<PoolEvent>,
    /// Sender for the multiplexed event channel (cloned to each worker bridge).
    event_tx: mpsc::UnboundedSender<PoolEvent>,
}

impl AgentPool {
    /// Create a new empty agent pool.
    #[must_use]
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            workers: HashMap::new(),
            topology: AgentTopology::new(),
            next_id: AtomicU32::new(1), // 0 reserved for supervisor
            event_rx,
            event_tx,
        }
    }

    /// Spawn a new worker agent with the given config and task.
    ///
    /// The `AgentSession` is started and moved into a background reader task.
    /// Messages are bridged into the pool's multiplexed event channel.
    /// Returns the assigned `AgentId`.
    pub async fn spawn_worker(
        &mut self,
        config: AgentConfig,
        task_id: String,
        task_description: String,
        scope: Vec<PathBuf>,
    ) -> Result<AgentId> {
        let id = AgentId::new(self.next_id.fetch_add(1, Ordering::SeqCst));
        let role = AgentRole::Worker {
            task_id: task_id.clone(),
        };

        let mut session = AgentSession::start(config).await?;

        // Cancel channel: the pool sends () to signal the worker to stop.
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel::<()>();

        // Bridge: forwards messages from the session to the pool's event channel.
        let bridge_tx = self.event_tx.clone();
        let bridge_id = id;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = cancel_rx.recv() => {
                        session.cancel();
                        // Drain remaining messages after cancel
                        while let Some(msg) = session.next_message().await {
                            if bridge_tx.send(PoolEvent {
                                agent_id: bridge_id,
                                message: msg,
                            }).is_err() {
                                break;
                            }
                        }
                        break;
                    }
                    msg = session.next_message() => {
                        match msg {
                            Some(m) => {
                                if bridge_tx.send(PoolEvent {
                                    agent_id: bridge_id,
                                    message: m,
                                }).is_err() {
                                    break;
                                }
                            }
                            None => break, // Session stream ended
                        }
                    }
                }
            }
        });

        self.topology.add(id, role.clone());
        self.workers.insert(
            id,
            WorkerHandle {
                cancel_tx,
                role,
                specialized_role: None,
                status: WorkerStatus::Running,
                task_description,
                scope,
                cost_usd: 0.0,
                tokens_used: 0,
            },
        );

        Ok(id)
    }

    /// Spawn a worker with a specialized role applied to the config.
    ///
    /// The role's system prompt, model preference, and tool policy are applied
    /// to the `AgentConfig` before starting the session.
    pub async fn spawn_worker_with_role(
        &mut self,
        config: AgentConfig,
        task_id: String,
        task_description: String,
        scope: Vec<PathBuf>,
        role: &SpecializedRole,
    ) -> Result<AgentId> {
        let config = role_config(config, role);

        let id = AgentId::new(self.next_id.fetch_add(1, Ordering::SeqCst));
        let agent_role = AgentRole::Worker {
            task_id: task_id.clone(),
        };

        let mut session = AgentSession::start(config).await?;

        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel::<()>();
        let bridge_tx = self.event_tx.clone();
        let bridge_id = id;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = cancel_rx.recv() => {
                        session.cancel();
                        while let Some(msg) = session.next_message().await {
                            if bridge_tx.send(PoolEvent {
                                agent_id: bridge_id,
                                message: msg,
                            }).is_err() {
                                break;
                            }
                        }
                        break;
                    }
                    msg = session.next_message() => {
                        match msg {
                            Some(m) => {
                                if bridge_tx.send(PoolEvent {
                                    agent_id: bridge_id,
                                    message: m,
                                }).is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        self.topology
            .add_with_specialization(id, agent_role.clone(), role.role_id.clone());
        self.workers.insert(
            id,
            WorkerHandle {
                cancel_tx,
                role: agent_role,
                specialized_role: Some(role.role_id.clone()),
                status: WorkerStatus::Running,
                task_description,
                scope,
                cost_usd: 0.0,
                tokens_used: 0,
            },
        );

        Ok(id)
    }

    /// Cancel a specific worker by ID.
    pub fn cancel_worker(&mut self, id: AgentId) {
        if let Some(handle) = self.workers.get_mut(&id)
            && !handle.status.is_terminal()
        {
            let _ = handle.cancel_tx.send(());
            handle.status = WorkerStatus::Cancelled;
        }
    }

    /// Cancel all non-terminal workers.
    pub fn cancel_all(&mut self) {
        let ids: Vec<AgentId> = self.workers.keys().copied().collect();
        for id in ids {
            self.cancel_worker(id);
        }
    }

    /// Get the next event from any worker in the pool.
    ///
    /// Returns `None` when all workers have completed and the channel is drained.
    pub async fn next_event(&mut self) -> Option<PoolEvent> {
        self.event_rx.recv().await
    }

    /// Update a worker's status.
    pub fn set_worker_status(&mut self, id: AgentId, status: WorkerStatus) {
        if let Some(handle) = self.workers.get_mut(&id) {
            handle.status = status;
        }
    }

    /// Update a worker's cost tracking.
    pub fn update_worker_cost(&mut self, id: AgentId, cost_usd: f64) {
        if let Some(handle) = self.workers.get_mut(&id) {
            handle.cost_usd = cost_usd;
        }
    }

    /// Update a worker's token usage.
    pub fn update_worker_tokens(&mut self, id: AgentId, tokens: u64) {
        if let Some(handle) = self.workers.get_mut(&id) {
            handle.tokens_used = tokens;
        }
    }

    /// Get a snapshot of all workers' statuses.
    #[must_use]
    pub fn status_snapshot(&self) -> Vec<WorkerSnapshot> {
        self.workers
            .iter()
            .map(|(&id, handle)| WorkerSnapshot {
                id,
                role: handle.role.clone(),
                specialized_role: handle.specialized_role.clone(),
                status: handle.status,
                task_description: handle.task_description.clone(),
                cost_usd: handle.cost_usd,
                tokens_used: handle.tokens_used,
            })
            .collect()
    }

    /// Get the number of active (non-terminal) workers.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.workers
            .values()
            .filter(|h| !h.status.is_terminal())
            .count()
    }

    /// Get the total number of workers (including terminal).
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.workers.len()
    }

    /// Whether all workers have reached a terminal state.
    #[must_use]
    pub fn is_all_done(&self) -> bool {
        !self.workers.is_empty() && self.workers.values().all(|h| h.status.is_terminal())
    }

    /// Get the topology.
    #[must_use]
    pub fn topology(&self) -> &AgentTopology {
        &self.topology
    }

    /// Get the aggregate cost across all workers.
    #[must_use]
    pub fn total_cost_usd(&self) -> f64 {
        self.workers.values().map(|h| h.cost_usd).sum()
    }

    /// Get the aggregate token usage across all workers.
    #[must_use]
    pub fn total_tokens_used(&self) -> u64 {
        self.workers.values().map(|h| h.tokens_used).sum()
    }
}

impl Default for AgentPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only snapshot of a worker's state.
#[derive(Debug, Clone)]
pub struct WorkerSnapshot {
    /// Agent ID.
    pub id: AgentId,
    /// Agent role.
    pub role: AgentRole,
    /// Specialized role ID (e.g., "researcher", "coder").
    pub specialized_role: Option<String>,
    /// Current status.
    pub status: WorkerStatus,
    /// Task description.
    pub task_description: String,
    /// Cumulative cost in USD.
    pub cost_usd: f64,
    /// Tokens consumed.
    pub tokens_used: u64,
}

/// Specialization changes capability preferences, preserving the caller's policy context.
fn role_config(mut config: AgentConfig, role: &SpecializedRole) -> AgentConfig {
    // Apply role's system prompt
    let rendered_prompt = role.render_prompt(&config.prompt);
    config.prompt = rendered_prompt;

    // Apply role's model preference
    if let Some(model) = role.effective_model() {
        config.model = Some(model.to_string());
    }

    // Apply role's tool access policy
    config.allowed_tools = apply_tool_policy(role, &config.allowed_tools);

    // Apply max iterations as max_turns
    if role.constraints.max_iterations > 0 {
        config.max_turns = Some(role.constraints.max_iterations);
    }

    config
}

#[cfg(test)]
#[path = "pool.tests.rs"]
mod tests;
