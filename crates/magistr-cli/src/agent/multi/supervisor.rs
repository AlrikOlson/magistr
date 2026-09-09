//! Supervisor agent — routes subtasks to workers, collects results, handles failures.
//!
//! The supervisor is the orchestrator that drives the multi-agent workflow:
//! 1. Takes a set of subtasks from the partitioning layer
//! 2. Spawns workers in the pool for each subtask
//! 3. Monitors worker events and updates status
//! 4. Handles failures with retries
//! 5. Collects results and produces a final summary

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use magistr_core::types::Archetype;

use super::budget::PoolBudget;
use super::canvas::CanvasEvent;
use super::conflict::ConflictDetector;
use super::context::SharedContext;
use super::partition::SubTask;
use super::pattern::{ExecutorCtx, execute_preset};
use super::pool::{AgentPool, PoolEvent};
use super::preset::OrchestratorPreset;
use super::role::SpecializedRole;
use super::role_dispatch::dispatch_to_role;
use super::topology::{AgentId, WorkerStatus};
use crate::agent::AgentConfig;
use crate::agent::message::AgentMessage;
use crate::error::Result;

// ─── Supervisor Events ─────────────────────────────────────────────────────

/// Events emitted by the supervisor for TUI consumption.
#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    /// A worker was spawned.
    WorkerSpawned { agent_id: AgentId, task_id: String },
    /// A worker completed successfully.
    WorkerCompleted { agent_id: AgentId, summary: String },
    /// A worker failed.
    WorkerFailed { agent_id: AgentId, error: String },
    /// A worker was cancelled.
    WorkerCancelled { agent_id: AgentId, reason: String },
    /// A worker is being retried.
    WorkerRetrying {
        agent_id: AgentId,
        attempt: u32,
        max_attempts: u32,
    },
    /// All workers have completed; orchestration is done.
    OrchestrationComplete {
        /// Number of successful workers.
        succeeded: usize,
        /// Number of failed workers.
        failed: usize,
        /// Total cost in USD.
        total_cost_usd: f64,
    },
    /// A conflict was detected between workers.
    ConflictDetected { file: PathBuf, agents: Vec<AgentId> },
    /// Budget limit reached for the pool.
    BudgetExceeded { reason: String },
}

// ─── Supervisor Config ─────────────────────────────────────────────────────

/// Configuration for the supervisor.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Working directory for agent sessions.
    pub cwd: PathBuf,
    /// Maximum retry attempts per worker.
    pub max_retries: u32,
    /// Pool-level budget constraints.
    pub budget: PoolBudget,
    /// Optional orchestration preset for pattern-aware execution.
    pub preset: Option<OrchestratorPreset>,
    /// Project archetypes for archetype-aware orchestration.
    pub archetypes: Vec<Archetype>,
}

impl SupervisorConfig {
    /// Create a new supervisor config with defaults.
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
            max_retries: 2,
            budget: PoolBudget::default(),
            preset: None,
            archetypes: Vec::new(),
        }
    }

    /// Set the maximum retry attempts.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set the pool budget.
    pub fn budget(mut self, budget: PoolBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Set the orchestration preset.
    pub fn preset(mut self, preset: OrchestratorPreset) -> Self {
        self.preset = Some(preset);
        self
    }

    /// Set the project archetypes for archetype-aware orchestration.
    pub fn archetypes(mut self, archetypes: Vec<Archetype>) -> Self {
        self.archetypes = archetypes;
        self
    }
}

// ─── Supervisor ────────────────────────────────────────────────────────────

/// Orchestrates multiple worker agents to complete a set of subtasks.
pub struct Supervisor {
    /// The agent pool managing worker sessions.
    pool: AgentPool,
    /// Supervisor configuration.
    config: SupervisorConfig,
    /// Conflict detector tracking file ownership.
    conflicts: ConflictDetector,
    /// Shared context for inter-agent knowledge sharing.
    context: SharedContext,
    /// Retry counters per original task_id.
    retry_counts: HashMap<String, u32>,
    /// Available specialized roles for dispatch.
    available_roles: Vec<SpecializedRole>,
    /// Channel for emitting supervisor events to the TUI.
    event_tx: mpsc::UnboundedSender<SupervisorEvent>,
    /// Receiver for supervisor events (taken by the TUI).
    event_rx: Option<mpsc::UnboundedReceiver<SupervisorEvent>>,
    /// Optional channel for canvas visualization events.
    canvas_tx: Option<mpsc::UnboundedSender<CanvasEvent>>,
}

impl Supervisor {
    /// Create a new supervisor.
    #[must_use]
    pub fn new(config: SupervisorConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            pool: AgentPool::new(),
            config,
            conflicts: ConflictDetector::new(),
            context: SharedContext::new(),
            retry_counts: HashMap::new(),
            available_roles: SpecializedRole::built_in_roles(),
            event_tx,
            event_rx: Some(event_rx),
            canvas_tx: None,
        }
    }

    /// Create a supervisor with custom available roles.
    #[must_use]
    pub fn with_roles(config: SupervisorConfig, roles: Vec<SpecializedRole>) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            pool: AgentPool::new(),
            config,
            conflicts: ConflictDetector::new(),
            context: SharedContext::new(),
            retry_counts: HashMap::new(),
            available_roles: roles,
            event_tx,
            event_rx: Some(event_rx),
            canvas_tx: None,
        }
    }

    /// Set the canvas event channel for visualization.
    pub fn set_canvas_tx(&mut self, tx: mpsc::UnboundedSender<CanvasEvent>) {
        self.canvas_tx = Some(tx);
    }

    /// Get the available specialized roles.
    #[must_use]
    pub fn available_roles(&self) -> &[SpecializedRole] {
        &self.available_roles
    }

    /// Take the supervisor event receiver.
    ///
    /// Can only be called once — subsequent calls return `None`.
    pub fn take_event_rx(&mut self) -> Option<mpsc::UnboundedReceiver<SupervisorEvent>> {
        self.event_rx.take()
    }

    /// Get a reference to the shared context.
    #[must_use]
    pub fn context(&self) -> &SharedContext {
        &self.context
    }

    /// Get a reference to the agent pool.
    #[must_use]
    pub fn pool(&self) -> &AgentPool {
        &self.pool
    }

    /// Run the orchestration using a pattern executor selected by the preset.
    ///
    /// If a preset is set in the config, delegates to the appropriate
    /// [`PatternExecutor`]. Otherwise falls through to the default `run()`.
    pub async fn run_with_preset(&mut self, subtasks: Vec<SubTask>) -> Result<Vec<WorkerResult>> {
        let preset = self.config.preset.clone();
        if let Some(ref p) = preset {
            let mut ctx = ExecutorCtx {
                pool: &mut self.pool,
                context: &self.context,
                conflicts: &mut self.conflicts,
                config: &self.config,
                roles: &self.available_roles,
                event_tx: &self.event_tx,
                canvas_tx: self.canvas_tx.as_ref(),
            };
            let results = execute_preset(p, &mut ctx, subtasks).await?;

            let succeeded = results.iter().filter(|r| r.success).count();
            let failed = results.iter().filter(|r| !r.success).count();
            let _ = self.event_tx.send(SupervisorEvent::OrchestrationComplete {
                succeeded,
                failed,
                total_cost_usd: self.pool.total_cost_usd(),
            });

            Ok(results)
        } else {
            self.run(subtasks).await
        }
    }

    /// Run the orchestration: spawn workers for all subtasks and monitor until completion.
    ///
    /// Returns the final result summaries from all workers.
    pub async fn run(&mut self, subtasks: Vec<SubTask>) -> Result<Vec<WorkerResult>> {
        // Spawn a worker for each subtask
        for task in &subtasks {
            self.spawn_for_subtask(task).await?;
        }

        let mut results: Vec<WorkerResult> = Vec::new();

        // Monitor workers until all are done
        while !self.pool.is_all_done() {
            // Check budget before processing next event
            if self
                .config
                .budget
                .is_exceeded(self.pool.total_cost_usd(), self.pool.total_tokens_used())
            {
                let _ = self.event_tx.send(SupervisorEvent::BudgetExceeded {
                    reason: "Pool budget exceeded".into(),
                });
                self.pool.cancel_all();
                break;
            }

            let event = match self.pool.next_event().await {
                Some(e) => e,
                None => break, // Channel closed
            };

            self.handle_pool_event(event, &subtasks, &mut results).await;
        }

        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        let _ = self.event_tx.send(SupervisorEvent::OrchestrationComplete {
            succeeded,
            failed,
            total_cost_usd: self.pool.total_cost_usd(),
        });

        Ok(results)
    }

    /// Spawn a worker for a single subtask, dispatching to the best-fit role.
    ///
    /// When the subtask carries an `archetype_id`, uses an archetype-specialized
    /// role with language-specific conventions, tool policies, and system prompt.
    async fn spawn_for_subtask(&mut self, task: &SubTask) -> Result<()> {
        let config =
            AgentConfig::new(&task.prompt, &self.config.cwd).with_project_context(&self.config.cwd);

        // Try archetype-specific role first, then generic role dispatch.
        let archetype_role = task.archetype_id.as_ref().and_then(|arch_id| {
            self.config
                .archetypes
                .iter()
                .find(|a| &a.id == arch_id)
                .map(|arch| {
                    let mut role = magistr_core::archetype_profile::archetype_coder(arch);
                    magistr_core::archetype_tool_policy::apply_archetype_tool_policy(
                        &mut role, arch,
                    );
                    role
                })
        });

        let id = if let Some(ref role) = archetype_role {
            self.pool
                .spawn_worker_with_role(
                    config,
                    task.task_id.clone(),
                    task.description.clone(),
                    task.scope.clone(),
                    role,
                )
                .await?
        } else if let Some(role) =
            dispatch_to_role(&task.task_id, &task.description, &self.available_roles)
        {
            self.pool
                .spawn_worker_with_role(
                    config,
                    task.task_id.clone(),
                    task.description.clone(),
                    task.scope.clone(),
                    role,
                )
                .await?
        } else {
            self.pool
                .spawn_worker(
                    config,
                    task.task_id.clone(),
                    task.description.clone(),
                    task.scope.clone(),
                )
                .await?
        };

        // Register file claims for conflict detection
        for path in &task.scope {
            self.conflicts.claim_file(id, path.clone());
        }

        let _ = self.event_tx.send(SupervisorEvent::WorkerSpawned {
            agent_id: id,
            task_id: task.task_id.clone(),
        });

        Ok(())
    }

    /// Handle a single pool event.
    async fn handle_pool_event(
        &mut self,
        event: PoolEvent,
        subtasks: &[SubTask],
        results: &mut Vec<WorkerResult>,
    ) {
        let agent_id = event.agent_id;

        match event.message {
            Ok(AgentMessage::Result {
                is_error,
                total_cost_usd,
                result,
                ..
            }) => {
                if let Some(cost) = total_cost_usd {
                    self.pool.update_worker_cost(agent_id, cost);
                }

                if is_error {
                    self.pool.set_worker_status(agent_id, WorkerStatus::Failed);
                    let error_msg = result.clone().unwrap_or_else(|| "unknown error".into());

                    // Check if we should retry
                    if let Some(task) = self.find_task_for_agent(agent_id, subtasks) {
                        let count = self.retry_counts.entry(task.task_id.clone()).or_insert(0);
                        if *count < self.config.max_retries {
                            *count += 1;
                            let attempt = *count;
                            let _ = self.event_tx.send(SupervisorEvent::WorkerRetrying {
                                agent_id,
                                attempt,
                                max_attempts: self.config.max_retries,
                            });
                            // Respawn for the same subtask
                            let _ = self.spawn_for_subtask(task).await;
                            return;
                        }
                    }

                    let _ = self.event_tx.send(SupervisorEvent::WorkerFailed {
                        agent_id,
                        error: error_msg.clone(),
                    });
                    results.push(WorkerResult {
                        agent_id,
                        success: false,
                        summary: error_msg,
                        cost_usd: total_cost_usd,
                    });
                } else {
                    self.pool
                        .set_worker_status(agent_id, WorkerStatus::Completed);
                    let summary = result.unwrap_or_default();
                    let _ = self.event_tx.send(SupervisorEvent::WorkerCompleted {
                        agent_id,
                        summary: summary.clone(),
                    });
                    results.push(WorkerResult {
                        agent_id,
                        success: true,
                        summary,
                        cost_usd: total_cost_usd,
                    });
                }
            }
            Ok(AgentMessage::UsageUpdate {
                context_tokens_used,
                cost_usd,
                ..
            }) => {
                self.pool
                    .update_worker_tokens(agent_id, context_tokens_used);
                if let Some(cost) = cost_usd {
                    self.pool.update_worker_cost(agent_id, cost);
                }
            }
            Ok(AgentMessage::Cancelled { reason, .. }) => {
                self.pool
                    .set_worker_status(agent_id, WorkerStatus::Cancelled);
                let _ = self
                    .event_tx
                    .send(SupervisorEvent::WorkerCancelled { agent_id, reason });
            }
            Ok(_) => {
                // Other messages (Assistant, User, StreamEvent, etc.) are
                // forwarded to the TUI via the pool event stream but don't
                // change supervisor state.
            }
            Err(e) => {
                self.pool.set_worker_status(agent_id, WorkerStatus::Failed);
                let _ = self.event_tx.send(SupervisorEvent::WorkerFailed {
                    agent_id,
                    error: e.to_string(),
                });
                results.push(WorkerResult {
                    agent_id,
                    success: false,
                    summary: e.to_string(),
                    cost_usd: None,
                });
            }
        }
    }

    /// Find the subtask associated with a given agent ID.
    fn find_task_for_agent<'a>(
        &self,
        agent_id: AgentId,
        subtasks: &'a [SubTask],
    ) -> Option<&'a SubTask> {
        let snapshot = self.pool.status_snapshot();
        let worker = snapshot.iter().find(|s| s.id == agent_id)?;
        let task_id = match &worker.role {
            super::topology::AgentRole::Worker { task_id } => task_id,
            _ => return None,
        };
        subtasks.iter().find(|t| &t.task_id == task_id)
    }
}

/// Result from a single worker.
#[derive(Debug, Clone)]
pub struct WorkerResult {
    /// Which agent produced this result.
    pub agent_id: AgentId,
    /// Whether the task succeeded.
    pub success: bool,
    /// Human-readable summary.
    pub summary: String,
    /// Cost in USD.
    pub cost_usd: Option<f64>,
}

#[cfg(test)]
#[path = "supervisor.tests.rs"]
mod tests;
