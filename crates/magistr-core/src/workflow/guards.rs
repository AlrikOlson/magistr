//! Orchestration lifecycle guards for the workflow engine.
//!
//! Abstract guard types (trait, result, context, config) live in `magistr-engine`.
//! This module re-exports them and provides the composable `GuardRunner`.

// Re-export abstract types from engine.
pub use magistr_engine::guard::{
    GuardAction, GuardContext, GuardResult, GuardsConfig, WorkflowGuard,
};

use crate::workflow::clean_tree_guard::CleanTreeGuard;
use crate::workflow::plan_guard::PlanGuard;
use crate::workflow::spec_guard::SpecGuard;

// ── GuardRunner ─────────────────────────────────────────────────────────────

/// Composable runner that executes a sequence of guards at each lifecycle point.
pub struct GuardRunner {
    guards: Vec<Box<dyn WorkflowGuard>>,
}

impl GuardRunner {
    /// Create a runner from a list of guards.
    pub fn new(guards: Vec<Box<dyn WorkflowGuard>>) -> Self {
        Self { guards }
    }

    /// Create a runner from configuration, instantiating only enabled guards.
    pub fn from_config(config: &GuardsConfig) -> Self {
        let mut guards: Vec<Box<dyn WorkflowGuard>> = Vec::new();
        if config.plan {
            guards.push(Box::new(PlanGuard));
        }
        if config.spec {
            guards.push(Box::new(SpecGuard));
        }
        if config.clean_tree {
            guards.push(Box::new(CleanTreeGuard));
        }
        if config.quality_gate {
            guards.push(Box::new(super::gate_guard::QualityGateGuard::new()));
        }
        Self { guards }
    }

    /// Run all guards' `pre_session` hooks, collecting non-ok results.
    pub fn run_pre_session(&self, ctx: &GuardContext) -> Vec<GuardResult> {
        self.guards
            .iter()
            .map(|g| g.pre_session(ctx))
            .filter(|r| !r.is_ok())
            .collect()
    }

    /// Run all guards' `post_session` hooks, collecting non-ok results.
    pub fn run_post_session(&self, ctx: &GuardContext) -> Vec<GuardResult> {
        self.guards
            .iter()
            .map(|g| g.post_session(ctx))
            .filter(|r| !r.is_ok())
            .collect()
    }

    /// Run all guards' `pre_commit` hooks, collecting non-ok results.
    pub fn run_pre_commit(&self, ctx: &GuardContext) -> Vec<GuardResult> {
        self.guards
            .iter()
            .map(|g| g.pre_commit(ctx))
            .filter(|r| !r.is_ok())
            .collect()
    }

    /// Run all guards' `post_commit` hooks, collecting non-ok results.
    pub fn run_post_commit(&self, ctx: &GuardContext) -> Vec<GuardResult> {
        self.guards
            .iter()
            .map(|g| g.post_commit(ctx))
            .filter(|r| !r.is_ok())
            .collect()
    }

    /// How many guards are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.guards.len()
    }

    /// Whether the runner has no guards.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "guards.tests.rs"]
mod tests;
