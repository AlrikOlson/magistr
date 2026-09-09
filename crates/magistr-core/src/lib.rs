//! magistr-core — frontend-agnostic domain types for Magistr.
//!
//! This crate defines shared types used by both the TUI (`magistr-cli`)
//! and GUI (`magistr-gui`) frontends. It has zero rendering dependencies —
//! no ratatui, no crossterm, no web framework.

// ── Core foundation (types, paths, env, error, detection) ───────────────────

pub mod child_guard;
pub mod confidence;
pub mod config;
pub mod detection;
pub mod env;
pub mod error;
pub mod paths;
pub mod types;

pub mod tools;
pub mod validate;
pub mod workbench_command;

// ── Orchestration (roadmap, workflow) ─────────────────────────────────────────

pub mod roadmap;
pub mod workflow;

// ── Multi-agent roles & recommendation ──────────────────────────────────────

pub mod archetype_profile;
pub mod archetype_tool_policy;
pub mod orchestration_preset;
pub mod role;
pub mod role_recommend;
pub mod role_recommendation;

// ── Agent & session domain ──────────────────────────────────────────────────

pub mod acp_session_state;
pub mod active_tool;
pub mod agent_message;
pub mod agent_types;
pub mod backend;
pub mod budget_state;
pub mod completed_tool;
pub mod content_block;
pub mod domain;
pub mod file_entry;
pub mod file_status;
pub mod gate;
pub mod gate_rows;
pub mod log;
pub mod loop_progress;
pub mod phase;
pub mod select;
pub mod session_id;
pub mod session_state;
pub mod state_update;
pub mod stream;
pub mod task;
pub mod tool_kind;
pub mod workflow_phase;
pub mod workflow_state;

// Re-export primary types for convenience.
pub use acp_session_state::AcpSessionState;
pub use active_tool::ActiveTool;
pub use agent_message::AgentMessage;
pub use agent_types::{
    AcpConfigOption, AcpPlanEntry, AgentCommand, ConfigOptionCategory, ConfigSelectValue,
    PendingTool, PlanEntryPriority, PlanEntryStatus, ToolLocation,
};
pub use backend::{Backend, BackendCapabilities};
pub use budget_state::BudgetState;
pub use completed_tool::{CompletedTool, MAX_COMPLETED_TOOLS};
pub use content_block::{AssistantContent, ContentBlock, UserBody, UserContent};
pub use domain::DomainState;
pub use file_entry::FileEntry;
pub use file_status::FileStatus;
pub use gate::{GATE_HISTORY_MAX, GateResult, GateSummary, IterationOutcome, IterationPhase};
pub use gate_rows::{GateFailureRow, GateRowSeverity, junit_failure_rows, parse_gate_rows};
pub use log::{LogLevel, LogMessage};
pub use loop_progress::LoopProgress;
pub use phase::Phase;
pub use select::{SelectItem, SelectList};
pub use session_id::SessionId;
pub use session_state::SessionState;
pub use state_update::{HandlerContext, StateUpdate, ToolResultLevel, UpdateBatch};
pub use stream::{StreamBlock, StyledSegment, TextStyle, TurnRole};
pub use task::{ActiveTask, TaskStatus};
pub use tool_kind::ToolKind;
pub use workflow_phase::WorkflowPhase;
pub use workflow_state::WorkflowState;

// Re-export multi-agent types.
pub use orchestration_preset::OrchestrationPreset;
pub use role::{RoleConstraints, SpecializedRole};
pub use role_recommend::{recommend_roles_for_task, recommend_roles_with_description};
pub use role_recommendation::RoleRecommendation;

// Re-export core foundation types.
pub use confidence::Confidence;
pub use detection::Detection;
pub use env::MagistrEnv;
pub use error::{MagistrError, MapMagistrErr};
pub use paths::ProjectPaths;
pub use types::{
    CiProvider, CustomGate, FormatConfig, FormatConfigOverride, Formatter, Framework, Language,
    Linter, PackageManager, PluginDetection, Preset, PresetLayer, PresetLayerType, PresetOverride,
    PresetPipeline, ProjectConfig, ProjectMeta, QualityGate, QualityGateOverride, SmartStack,
    SpecConfig, Stack, StackConfig, TestFramework, TestingConfig, TestingConfigOverride, Workspace,
};
