//! Agent topology model — defines roles, identities, and the inter-agent message protocol.
//!
//! This module is pure data types with no I/O or async. It defines the
//! vocabulary for multi-agent orchestration: who the agents are, what roles
//! they play, and how they communicate.

use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ─── Agent Identity ────────────────────────────────────────────────────────

/// Unique identifier for an agent within a pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(u32);

impl AgentId {
    /// Create a new agent ID.
    #[must_use]
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw numeric ID.
    #[must_use]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent-{}", self.0)
    }
}

// ─── Roles ─────────────────────────────────────────────────────────────────

/// The role an agent plays in the orchestration topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// Coordinates workers, routes tasks, collects results.
    Supervisor,
    /// Executes a subtask assigned by the supervisor.
    Worker {
        /// Identifier for the subtask this worker is executing.
        task_id: String,
    },
    /// Combines results from multiple workers into a unified output.
    Merger,
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supervisor => write!(f, "supervisor"),
            Self::Worker { task_id } => write!(f, "worker({task_id})"),
            Self::Merger => write!(f, "merger"),
        }
    }
}

// ─── Worker Status ─────────────────────────────────────────────────────────

/// Lifecycle status of a worker agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// Spawned but not yet assigned work.
    Idle,
    /// Actively processing a subtask.
    Running,
    /// Completed its subtask successfully.
    Completed,
    /// Failed with an error.
    Failed,
    /// Cancelled by the supervisor or budget enforcer.
    Cancelled,
}

impl WorkerStatus {
    /// Whether this status represents a terminal state.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

// ─── Inter-Agent Messages ──────────────────────────────────────────────────

/// Messages exchanged between agents in the orchestration topology.
///
/// These are logical messages — they don't go over the wire. The supervisor
/// translates them into ACP session operations (prompts, cancels, etc.).
#[derive(Debug, Clone)]
pub enum AgentMsg {
    /// Supervisor assigns a subtask to a worker.
    TaskAssignment {
        /// Target worker.
        target: AgentId,
        /// Subtask description (becomes the agent prompt).
        prompt: String,
        /// Files/directories this worker is scoped to.
        scope: Vec<PathBuf>,
    },

    /// Worker reports its result back to the supervisor.
    TaskResult {
        /// Reporting worker.
        source: AgentId,
        /// Whether the task succeeded.
        success: bool,
        /// Human-readable result summary.
        summary: String,
        /// Files modified by this worker.
        files_modified: Vec<PathBuf>,
        /// Cost in USD for this worker's session.
        cost_usd: Option<f64>,
    },

    /// Worker publishes a status update.
    StatusUpdate {
        /// Reporting worker.
        source: AgentId,
        /// New status.
        status: WorkerStatus,
        /// Optional detail message.
        detail: Option<String>,
    },

    /// Worker shares a finding with others via the shared context.
    ContextShare {
        /// Publishing worker.
        source: AgentId,
        /// Key for the finding.
        key: String,
        /// Value (free-form text).
        value: String,
    },

    /// Supervisor requests a worker to cancel.
    CancelRequest {
        /// Target worker.
        target: AgentId,
        /// Reason for cancellation.
        reason: String,
    },
}

impl AgentMsg {
    /// Get the source agent ID, if this message has one.
    #[must_use]
    pub fn source(&self) -> Option<AgentId> {
        match self {
            Self::TaskResult { source, .. }
            | Self::StatusUpdate { source, .. }
            | Self::ContextShare { source, .. } => Some(*source),
            Self::TaskAssignment { .. } | Self::CancelRequest { .. } => None,
        }
    }

    /// Get the target agent ID, if this message has one.
    #[must_use]
    pub fn target(&self) -> Option<AgentId> {
        match self {
            Self::TaskAssignment { target, .. } | Self::CancelRequest { target, .. } => {
                Some(*target)
            }
            Self::TaskResult { .. } | Self::StatusUpdate { .. } | Self::ContextShare { .. } => None,
        }
    }
}

// ─── Topology ──────────────────────────────────────────────────────────────

/// Describes the agent topology — which agents exist and their roles.
#[derive(Debug, Clone)]
pub struct AgentTopology {
    /// Agent entries in the topology.
    entries: Vec<TopologyEntry>,
}

/// A single agent in the topology.
#[derive(Debug, Clone)]
pub struct TopologyEntry {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Role in the orchestration.
    pub role: AgentRole,
    /// Specialized role ID (e.g., "researcher", "coder"). `None` for non-specialized agents.
    pub specialized_role: Option<String>,
    /// When this entry was created.
    pub created_at: Instant,
}

impl AgentTopology {
    /// Create a new empty topology.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an agent to the topology.
    pub fn add(&mut self, id: AgentId, role: AgentRole) {
        self.entries.push(TopologyEntry {
            id,
            role,
            specialized_role: None,
            created_at: Instant::now(),
        });
    }

    /// Add an agent with a specialized role to the topology.
    pub fn add_with_specialization(
        &mut self,
        id: AgentId,
        role: AgentRole,
        specialized_role: String,
    ) {
        self.entries.push(TopologyEntry {
            id,
            role,
            specialized_role: Some(specialized_role),
            created_at: Instant::now(),
        });
    }

    /// Remove an agent from the topology.
    pub fn remove(&mut self, id: AgentId) {
        self.entries.retain(|e| e.id != id);
    }

    /// Get an agent's entry by ID.
    #[must_use]
    pub fn get(&self, id: AgentId) -> Option<&TopologyEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get all agents with a specific role.
    pub fn agents_with_role(&self, role_match: &AgentRole) -> Vec<&TopologyEntry> {
        self.entries
            .iter()
            .filter(|e| std::mem::discriminant(&e.role) == std::mem::discriminant(role_match))
            .collect()
    }

    /// Get the number of agents in the topology.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the topology is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the number of workers.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.role, AgentRole::Worker { .. }))
            .count()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &TopologyEntry> {
        self.entries.iter()
    }
}

impl Default for AgentTopology {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "topology.tests.rs"]
mod tests;
