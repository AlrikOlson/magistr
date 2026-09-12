//! Magistr orchestration and native workflow integration.
//!
//! Deterministic policy, run rules and gate values live in `magistr-domain`;
//! compatibility exports here preserve existing callers. This crate also owns
//! runtime and native-store adapters and is not an effect-free boundary.
//!
//! It provides backend-agnostic services and values for:
//! - **Session lifecycle** — `EngineSession`, `SessionStatus`, `SessionEvent`, `SessionCommand`
//! - **Session migration** — `SessionSnapshot`, `SessionResidency`, `RestoredSession`
//! - **Workflow orchestration** — `WorkflowEngine`, `WorkflowPhase`, `WorkflowConfig`
//! - **Cost governance** — `CostGovernor`, `BudgetConfig`, `BudgetStatus`, `CostEvent`
//! - **Workflow policies** — `GovernancePolicy`, `PolicyEngine`, `PolicyRule`, `PolicyEffect`
//! - **Multi-project batches** — `BatchRun`, `ProjectGraph`, `ProjectDep`, `BatchStatus`
//! - **Fleet operations** — `FleetRegistry`, `FleetNode`, `NodeHealth`, `PolicyBundle`, `FleetStatus`
//! - **Roadmap tracking** — `RoadmapManager`, `RoadmapProgress`, `RoadmapEvent`
//! - **Project context** — `ProjectContext` (immutable snapshot)
//! - **Engine handle** — `EngineHandle` (thread-safe entry point composing all services)
//! - **Quality gates** — `GateKind`, `GateStatus`, `GateOutcome`
//! - **Workflow guards** — `WorkflowGuard`, `GuardResult`, `GuardsConfig`, `ShipGuardState`
//! - **Spec parsing** — `SpecSummary`
//! - **Prompt building** — `PromptConfig`
//! - **Error handling** — `EngineError`
//! - **Native think-and-ship** — `native::ProjectAuthority` (admitted writer), `native::NativeEngines`,
//!   `native::NativeReader`, `native::NativeConfig`, `native::RoadmapBackend`
//!
//! Zero TUI dependencies. No ratatui, crossterm, or tachyonfx.
//! Infrastructure adapters in `magistr-cli` bridge these domain types
//! to the concrete ACP protocol and terminal UI.

pub mod batch;
pub mod cost;
pub mod error;
pub mod fleet;
pub mod gate;
pub mod guard;
pub mod handle;
pub mod native;
pub mod peer;
pub mod policy;
pub mod project;
pub mod prompt;
pub mod roadmap;
pub mod session;
pub mod spec;
pub mod workflow;

// Re-export primary types for convenience.
pub use batch::{BatchRow, BatchRun, BatchState, BatchStatus, ProjectDep, ProjectGraph};
pub use cost::{
    BudgetConfig, BudgetDecision, BudgetStatus, CostAttribution, CostEvent, CostGovernor,
    CostSample, TokenType,
};
pub use error::EngineError;
pub use fleet::{
    FleetNode, FleetRegistry, FleetStatus, Job, JobState, NodeHealth, PolicyBundle, StreamRelay,
    WorkQueue, WorkUnit,
};
pub use gate::{GateKind, GateOutcome, GateStatus};
pub use guard::{
    GuardAction, GuardContext, GuardResult, GuardsConfig, ShipGuardState, WorkflowGuard,
};
pub use handle::{EngineEvent, EngineHandle, WorkflowEvent};
pub use native::{
    AttachError, AttachKind, AttachRefusal, CallerIdentity, Delivery, EventCursor, FeedCursor,
    FeedStatus, GateOutput, MagistrFrame, McpAcceptor, NativeConfig, NativeEngines, NativeEvent,
    NativeFrame, NativeReader, NativeRoadmapSnapshot, NativeSnapshot, OwnerClient, OwnerCommand,
    OwnerEndpoint, OwnerGeneration, OwnerReply, ProjectAuthority, RoadmapBackend, RoadmapCommand,
    RoadmapFrame, ShipCommand, ShipFrame, SignalCommand, SignalFrame, SnapshotCursor, ThinkCommand,
    ThinkFrame, WriterRole,
};
pub use policy::{
    GovernancePolicy, ModelTier, ModelTierRule, PolicyCondition, PolicyDecision, PolicyEffect,
    PolicyEngine, PolicyRequest, PolicyRule, RiskTier,
};
pub use project::ProjectContext;
pub use prompt::PromptConfig;
pub use roadmap::{RoadmapEvent, RoadmapManager, RoadmapProgress};
pub use session::{
    CheckpointReason, EngineSession, RepoState, RestoredSession, SessionCommand, SessionCost,
    SessionEvent, SessionResidency, SessionSnapshot, SessionStatus,
};
pub use spec::SpecSummary;
// The library's own wire types behind the NativeEngines verbs, re-exported
// so engine hosts (the TUI, the GUI) can drive them without depending on
// think-and-ship directly.
pub use magistr_workflow::roadmap::domain::ChunkStatus;
pub use magistr_workflow::ship::domain::action::ActionType;
pub use magistr_workflow::ship::domain::check::CheckType;
pub use magistr_workflow::ship::domain::task::TaskType;
pub use magistr_workflow::signal::domain::SignalKind;
pub use magistr_workflow::think::domain::ThinkStep;
pub use workflow::{
    HookAction, HookDispatcher, HookError, HookExecutionReport, HookExecutor, HookOutput,
    HookResolver, IterationState, IterationStatus, LifecycleEvent, LifecycleHook,
    LifecycleHooksConfig, OnFailure, QualityGateConfig, ResolvedHook, WorkflowCommand,
    WorkflowConfig, WorkflowEngine, WorkflowPhase,
};

#[cfg(test)]
#[path = "lib.tests.rs"]
mod tests;
