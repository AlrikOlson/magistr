//! Workflow engine — domain service for plan-execute-verify orchestration.
//!
//! `WorkflowPhase` and `WorkflowEngine` define the pure domain logic for
//! workflow phase transitions. No filesystem, no process spawning.

pub mod hook_executor;
pub mod hook_presets;
pub mod hook_resolver;
pub mod iteration_state;
pub mod iteration_status;
pub mod lifecycle_event;
pub mod lifecycle_hook;
pub mod lifecycle_hooks_config;
pub mod quality_gate_config;
pub mod workflow_command;
pub mod workflow_config;
pub mod workflow_engine;
pub mod workflow_phase;

pub use hook_executor::{HookDispatcher, HookError, HookExecutionReport, HookExecutor, HookOutput};
pub use hook_resolver::HookResolver;
pub use iteration_state::IterationState;
pub use iteration_status::IterationStatus;
pub use lifecycle_event::LifecycleEvent;
pub use lifecycle_hook::{HookAction, LifecycleHook, OnFailure, ResolvedHook};
pub use lifecycle_hooks_config::LifecycleHooksConfig;
pub use quality_gate_config::QualityGateConfig;
pub use workflow_command::WorkflowCommand;
pub use workflow_config::WorkflowConfig;
pub use workflow_engine::WorkflowEngine;
pub use workflow_phase::WorkflowPhase;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
