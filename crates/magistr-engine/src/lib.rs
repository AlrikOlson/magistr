//! Magistr orchestration and native workflow integration.
//!
//! Deterministic policy, run rules and gate values live in `magistr-domain`;
//! this crate currently re-exports those values for its callers. It also owns
//! runtime and native-store adapters and is not an effect-free boundary.
//!
//! It provides backend-agnostic services and values for:
//! - **Session lifecycle** — `EngineSession`, `SessionStatus`, `SessionEvent`, `SessionCommand`
//! - **Session migration** — `SessionSnapshot`, `SessionResidency`, `RestoredSession`
//! - **Workflow orchestration** — `WorkflowEngine`, `WorkflowPhase`, `WorkflowConfig`
//! - **Cost governance** — `CostGovernor`, `BudgetConfig`, `BudgetStatus`, `CostEvent`
//! - **Workflow policies** — `GovernancePolicy`, `PolicyEngine`, `PolicyRule`, `PolicyEffect`
//! - **Plan tracking** — `PlanManager`, `PlanProgress`, `PlanEvent`
//! - **Engine handle** — `EngineHandle` (thread-safe entry point composing all services)
//! - **Quality gates** — `GateKind`, `GateStatus`, `GateOutcome`
//! - **Workflow guards** — `WorkflowGuard`, `GuardResult`, `GuardsConfig`, `RunGuardState`
//! - **Spec parsing** — `SpecSummary`
//! - **Prompt building** — `PromptConfig`
//! - **Error handling** — `EngineError`
//! - **Native magistr** — `native::ProjectAuthority` (admitted writer), `native::NativeEngines`,
//!   `native::NativeReader`, `native::NativeConfig`, `native::PlanBackend`
//!
//! Zero TUI dependencies. No ratatui, crossterm, or tachyonfx.
//! Infrastructure adapters in `magistr-cli` bridge these domain types
//! to the concrete ACP protocol and terminal UI.

pub mod cost;
pub mod error;
pub mod gate;
pub mod guard;
pub mod handle;
pub mod native;
/// Experimental peer reasoning only; no runtime authentication or dispatch.
#[cfg(any(test, feature = "experimental-peer"))]
pub mod peer;
pub mod plan;
pub mod policy;
pub mod prompt;
pub mod session;
pub mod spec;
pub mod workflow;

// Re-export primary types for convenience.
pub use cost::{
    BudgetConfig, BudgetDecision, BudgetStatus, CostAttribution, CostEvent, CostGovernor,
    CostSample, TokenType,
};
pub use error::EngineError;
pub use gate::{GateKind, GateOutcome, GateStatus};
pub use guard::{
    GuardAction, GuardContext, GuardResult, GuardsConfig, RunGuardState, WorkflowGuard,
};
pub use handle::{EngineEvent, EngineHandle, WorkflowEvent};
pub use native::{
    AttachError, AttachKind, AttachRefusal, CallerIdentity, Delivery, EventCursor, FeedCursor,
    FeedStatus, GateOutput, MagistrFrame, McpAcceptor, NativeConfig, NativeEngines, NativeEvent,
    NativeFrame, NativePlanSnapshot, NativeReader, NativeSnapshot, Observed, OwnerClient,
    OwnerCommand, OwnerEndpoint, OwnerGeneration, OwnerReply, PlanBackend, PlanCommand, PlanData,
    PlanFrame, ProjectAuthority, ReadError, ReadErrorKind, ReadSource, ReadState, Run, RunCommand,
    RunData, RunFrame, SignalCommand, SignalFrame, StoreStamp, ThinkCommand, ThinkData, ThinkFrame,
    WriterRole,
};
pub use plan::{PlanEvent, PlanManager, PlanProgress};
pub use policy::{
    GovernancePolicy, ModelTier, ModelTierRule, PolicyCondition, PolicyDecision, PolicyEffect,
    PolicyEngine, PolicyRequest, PolicyRule, RiskTier,
};
pub use prompt::PromptConfig;
pub use session::{
    CheckpointReason, EngineSession, RepoState, RestoredSession, SessionCommand, SessionCost,
    SessionEvent, SessionResidency, SessionSnapshot, SessionStatus,
};
pub use spec::SpecSummary;
// The library's own wire types behind the NativeEngines verbs, re-exported
// so engine hosts (the interface, the GUI) can drive them without depending on
// magistr directly.
pub use magistr_workflow::plan::domain::TaskStatus;
pub use magistr_workflow::run::domain::check::CheckType;
pub use magistr_workflow::run::domain::progress::ProgressType;
pub use magistr_workflow::run::domain::step::StepType;
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
