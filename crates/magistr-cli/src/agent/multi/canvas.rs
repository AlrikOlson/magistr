//! Canvas data model for visualizing multi-agent orchestration topologies.
//!
//! Pure data types — no I/O, no async. These types describe the visual state
//! of agents and their connections on an interactive canvas. They are designed
//! to be serialized across the Tauri bridge to the Svelte frontend.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::topology::{AgentId, AgentRole};

// ─── Node Status ───────────────────────────────────────────────────────────

/// Visual status of an agent node on the canvas.
///
/// This is richer than `WorkerStatus` — it captures what the agent is
/// *currently doing* for animation purposes, not just lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Agent is idle — no active work.
    Idle,
    /// Agent is thinking / generating a response.
    Thinking,
    /// Agent is executing a tool call.
    ToolCall,
    /// Agent is writing / streaming text output.
    Writing,
    /// Agent encountered an error.
    Error,
}

impl NodeStatus {
    /// Whether this status should trigger a glow effect on the node.
    #[must_use]
    pub fn is_glowing(self) -> bool {
        matches!(self, Self::ToolCall)
    }

    /// Whether this status should trigger a pulse animation.
    #[must_use]
    pub fn is_pulsing(self) -> bool {
        matches!(self, Self::Error)
    }
}

// ─── Canvas Position ───────────────────────────────────────────────────────

/// 2D position on the canvas in logical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanvasPosition {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

impl CanvasPosition {
    /// Create a new position.
    #[must_use]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Distance to another position.
    #[must_use]
    pub fn distance_to(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

impl Default for CanvasPosition {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

// ─── Token Usage ───────────────────────────────────────────────────────────

/// Token consumption for an agent, used to render the burn gauge arc.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens consumed so far.
    pub used: u64,
    /// Token budget for this agent (0 = unlimited).
    pub budget: u64,
}

impl TokenUsage {
    /// Create a new token usage tracker.
    #[must_use]
    pub fn new(used: u64, budget: u64) -> Self {
        Self { used, budget }
    }

    /// Fraction of budget consumed (0.0–1.0). Returns 0.0 if budget is 0.
    #[must_use]
    pub fn fraction(self) -> f64 {
        if self.budget == 0 {
            return 0.0;
        }
        (self.used as f64 / self.budget as f64).min(1.0)
    }

    /// Whether usage has exceeded the budget.
    #[must_use]
    pub fn is_exceeded(self) -> bool {
        self.budget > 0 && self.used > self.budget
    }
}

// ─── Canvas Node ───────────────────────────────────────────────────────────

/// A node on the agent canvas representing a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasNode {
    /// Unique agent identifier.
    pub agent_id: AgentId,
    /// Role in the orchestration topology.
    pub role: AgentRole,
    /// Position on the canvas.
    pub position: CanvasPosition,
    /// Current visual status.
    pub status: NodeStatus,
    /// Token consumption.
    pub token_usage: TokenUsage,
    /// Name of the currently active tool, if any.
    pub active_tool: Option<String>,
    /// Specialized role ID (e.g., "researcher", "coder").
    pub specialized_role: Option<String>,
    /// Display name for the agent.
    pub display_name: String,
    /// Model identifier (e.g., "claude-sonnet-4-5").
    pub model: Option<String>,
    /// Whether the user has manually pinned this node's position.
    pub pinned: bool,
}

impl CanvasNode {
    /// Create a new canvas node with default position and idle status.
    #[must_use]
    pub fn new(agent_id: AgentId, role: AgentRole, display_name: String) -> Self {
        Self {
            agent_id,
            role,
            position: CanvasPosition::default(),
            status: NodeStatus::Idle,
            token_usage: TokenUsage::default(),
            active_tool: None,
            specialized_role: None,
            display_name,
            model: None,
            pinned: false,
        }
    }

    /// Set the position.
    #[must_use]
    pub fn with_position(mut self, x: f64, y: f64) -> Self {
        self.position = CanvasPosition::new(x, y);
        self
    }

    /// Set the specialized role.
    #[must_use]
    pub fn with_specialized_role(mut self, role: String) -> Self {
        self.specialized_role = Some(role);
        self
    }

    /// Set the model.
    #[must_use]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }
}

// ─── Edge Message Type ─────────────────────────────────────────────────────

/// Type of message flowing along a canvas edge.
///
/// Determines the edge color and particle animation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeMessageType {
    /// Supervisor delegates a task to a worker.
    TaskDelegation,
    /// Worker reports a result back.
    Result,
    /// Conflict signal between agents editing the same file.
    ConflictSignal,
    /// Shared context update broadcast.
    ContextUpdate,
}

// ─── Animation State ───────────────────────────────────────────────────────

/// Animation state for an edge's message flow visualization.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeAnimationState {
    /// Number of particles currently flowing along the edge.
    pub particle_count: u32,
    /// Speed multiplier for particle animation (1.0 = normal).
    pub speed: f64,
    /// Whether the animation is currently active.
    pub active: bool,
}

impl Default for EdgeAnimationState {
    fn default() -> Self {
        Self {
            particle_count: 0,
            speed: 1.0,
            active: false,
        }
    }
}

// ─── Canvas Edge ───────────────────────────────────────────────────────────

/// A directed edge between two agent nodes on the canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEdge {
    /// Source agent (message sender).
    pub source: AgentId,
    /// Target agent (message receiver).
    pub target: AgentId,
    /// Type of message flowing along this edge.
    pub message_type: EdgeMessageType,
    /// When this edge was last active.
    pub last_active: SystemTime,
    /// Animation state for the particle flow.
    pub animation: EdgeAnimationState,
    /// Cumulative message count (drives edge thickness).
    pub message_count: u32,
}

impl CanvasEdge {
    /// Create a new canvas edge.
    #[must_use]
    pub fn new(source: AgentId, target: AgentId, message_type: EdgeMessageType) -> Self {
        Self {
            source,
            target,
            message_type,
            last_active: SystemTime::now(),
            animation: EdgeAnimationState::default(),
            message_count: 0,
        }
    }

    /// Record a new message flowing along this edge.
    pub fn record_message(&mut self) {
        self.message_count = self.message_count.saturating_add(1);
        self.last_active = SystemTime::now();
        self.animation.active = true;
        // Scale particles with message volume (1–5 range).
        self.animation.particle_count = self.message_count.clamp(1, 5);
    }

    /// Normalized thickness factor based on message count (1.0–3.0 range).
    #[must_use]
    pub fn thickness_factor(&self) -> f64 {
        // Logarithmic scaling: 1 msg → 1.0, ~20 msgs → 3.0
        1.0 + (self.message_count as f64).ln_1p() * 0.67
    }
}

// ─── Canvas Events ─────────────────────────────────────────────────────────

/// Events that update the canvas state.
///
/// These are emitted by the orchestration layer and consumed by the
/// canvas renderer (TUI or GUI) to update the visual representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanvasEvent {
    /// A node was added to the canvas (agent spawned).
    NodeAdded { node: CanvasNode },

    /// A node was removed from the canvas (agent terminated).
    NodeRemoved { agent_id: AgentId },

    /// An agent's status changed (idle → thinking → tool-call → etc.).
    StatusChanged {
        agent_id: AgentId,
        status: NodeStatus,
        active_tool: Option<String>,
    },

    /// An agent's token usage was updated.
    TokenUsageUpdated {
        agent_id: AgentId,
        usage: TokenUsage,
    },

    /// A message was sent between agents (adds/updates an edge).
    MessageSent {
        source: AgentId,
        target: AgentId,
        message_type: EdgeMessageType,
    },

    /// A conflict was detected between agents.
    ConflictDetected { file: PathBuf, agents: Vec<AgentId> },

    /// The full topology was replaced (e.g., preset change).
    TopologyReset {
        nodes: Vec<CanvasNode>,
        edges: Vec<CanvasEdge>,
    },

    /// A node's position was updated (drag or layout).
    NodeMoved {
        agent_id: AgentId,
        position: CanvasPosition,
        pinned: bool,
    },
}

#[cfg(test)]
#[path = "canvas.tests.rs"]
mod tests;
