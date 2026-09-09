//! Pattern executors — orchestration-pattern-specific execution strategies.
//!
//! Each [`OrchestratorPreset`] maps to a pattern executor that drives the
//! multi-agent workflow with the correct sequencing, handoff, and monitoring
//! logic for that pattern. Access executors through [`executor_for_preset`].

use std::collections::HashMap;

use tokio::sync::mpsc;

use super::canvas::{CanvasEvent, EdgeMessageType};
use super::conflict::ConflictDetector;
use super::context::SharedContext;
use super::partition::SubTask;
use super::pool::{AgentPool, PoolEvent};
use super::preset::OrchestratorPreset;
use super::role::SpecializedRole;
use super::role_dispatch::dispatch_to_role;
use super::supervisor::{SupervisorConfig, SupervisorEvent, WorkerResult};
use super::topology::{AgentId, WorkerStatus};
use crate::agent::AgentConfig;
use crate::agent::message::AgentMessage;
use crate::error::Result;

// ─── Executor Context ──────────────────────────────────────────────────────

/// Bundles mutable orchestration state passed to pattern executors.
///
/// This avoids passing many individual parameters through the trait boundary.
pub struct ExecutorCtx<'a> {
    pub pool: &'a mut AgentPool,
    pub context: &'a SharedContext,
    pub conflicts: &'a mut ConflictDetector,
    pub config: &'a SupervisorConfig,
    pub roles: &'a [SpecializedRole],
    pub event_tx: &'a mpsc::UnboundedSender<SupervisorEvent>,
    /// Optional channel for canvas visualization events.
    pub canvas_tx: Option<&'a mpsc::UnboundedSender<CanvasEvent>>,
}

impl<'a> ExecutorCtx<'a> {
    /// Emit a canvas event if the canvas channel is connected.
    fn emit_canvas(&self, event: CanvasEvent) {
        if let Some(tx) = self.canvas_tx {
            let _ = tx.send(event);
        }
    }

    /// Emit a message-flow canvas event between two agents.
    fn emit_message_flow(&self, source: AgentId, target: AgentId, msg_type: EdgeMessageType) {
        self.emit_canvas(CanvasEvent::MessageSent {
            source,
            target,
            message_type: msg_type,
        });
    }
}

// ─── Pattern Executor Trait ────────────────────────────────────────────────

/// Drives a multi-agent workflow with pattern-specific orchestration logic.
trait PatternExecutor: Send + Sync {
    /// Execute the orchestration pattern with the given subtasks.
    fn execute(
        &self,
        ctx: &mut ExecutorCtx<'_>,
        subtasks: Vec<SubTask>,
    ) -> impl std::future::Future<Output = Result<Vec<WorkerResult>>> + Send;
}

// ─── Factory ───────────────────────────────────────────────────────────────

/// Execute the appropriate pattern for a given orchestration preset.
///
/// This is the main entry point — it selects the correct executor for the
/// preset and immediately runs it with the provided context.
pub async fn execute_preset(
    preset: &OrchestratorPreset,
    ctx: &mut ExecutorCtx<'_>,
    subtasks: Vec<SubTask>,
) -> Result<Vec<WorkerResult>> {
    match preset {
        OrchestratorPreset::ParallelTdd => ParallelTddExecutor.execute(ctx, subtasks).await,
        OrchestratorPreset::ResearchThenImplement => {
            ResearchThenImplementExecutor.execute(ctx, subtasks).await
        }
        OrchestratorPreset::ReviewPair => {
            ReviewPairExecutor::default().execute(ctx, subtasks).await
        }
        // Multi-archetype presets: fullstack-feature and test-across-stacks
        // run all archetype subtasks in parallel (same as ParallelTdd).
        OrchestratorPreset::FullstackFeature | OrchestratorPreset::TestAcrossStacks => {
            ParallelTddExecutor.execute(ctx, subtasks).await
        }
        // API-first: sequential handoff — backend archetypes first, then frontend.
        OrchestratorPreset::ApiFirst => ResearchThenImplementExecutor.execute(ctx, subtasks).await,
    }
}

// ─── Shared Helpers ────────────────────────────────────────────────────────

/// Spawn a single subtask into the pool, using role dispatch if possible.
async fn spawn_subtask(ctx: &mut ExecutorCtx<'_>, task: &SubTask) -> Result<AgentId> {
    let agent_config =
        AgentConfig::new(&task.prompt, &ctx.config.cwd).with_project_context(&ctx.config.cwd);

    let id = if let Some(role) = dispatch_to_role(&task.task_id, &task.description, ctx.roles) {
        ctx.pool
            .spawn_worker_with_role(
                agent_config,
                task.task_id.clone(),
                task.description.clone(),
                task.scope.clone(),
                role,
            )
            .await?
    } else {
        ctx.pool
            .spawn_worker(
                agent_config,
                task.task_id.clone(),
                task.description.clone(),
                task.scope.clone(),
            )
            .await?
    };

    for path in &task.scope {
        ctx.conflicts.claim_file(id, path.clone());
    }

    let _ = ctx.event_tx.send(SupervisorEvent::WorkerSpawned {
        agent_id: id,
        task_id: task.task_id.clone(),
    });

    // Emit canvas delegation flow: Supervisor(0) → Worker.
    let supervisor_id = AgentId::new(0);
    ctx.emit_message_flow(supervisor_id, id, EdgeMessageType::TaskDelegation);

    Ok(id)
}

/// State carried through the event-processing loop.
struct DrainState {
    retry_counts: HashMap<String, u32>,
    results: Vec<WorkerResult>,
}

impl DrainState {
    fn new() -> Self {
        Self {
            retry_counts: HashMap::new(),
            results: Vec::new(),
        }
    }
}

/// Drain pool events until all workers reach a terminal state.
async fn drain_until_done(ctx: &mut ExecutorCtx<'_>, state: &mut DrainState, subtasks: &[SubTask]) {
    while !ctx.pool.is_all_done() {
        if budget_exceeded(ctx) {
            break;
        }

        let event = match ctx.pool.next_event().await {
            Some(e) => e,
            None => break,
        };

        handle_pool_event(ctx, event, state, subtasks).await;
    }
}

/// Drain events until specific agents reach a terminal state.
async fn drain_for_agents(
    ctx: &mut ExecutorCtx<'_>,
    target_ids: &[AgentId],
    state: &mut DrainState,
    subtasks: &[SubTask],
) {
    loop {
        let snapshot = ctx.pool.status_snapshot();
        let all_done = target_ids.iter().all(|id| {
            snapshot
                .iter()
                .find(|s| s.id == *id)
                .is_some_and(|s| s.status.is_terminal())
        });
        if all_done {
            break;
        }

        if budget_exceeded(ctx) {
            break;
        }

        let event = match ctx.pool.next_event().await {
            Some(e) => e,
            None => break,
        };

        handle_pool_event(ctx, event, state, subtasks).await;
    }
}

/// Check budget and cancel all workers if exceeded. Returns true if exceeded.
fn budget_exceeded(ctx: &mut ExecutorCtx<'_>) -> bool {
    if ctx
        .config
        .budget
        .is_exceeded(ctx.pool.total_cost_usd(), ctx.pool.total_tokens_used())
    {
        let _ = ctx.event_tx.send(SupervisorEvent::BudgetExceeded {
            reason: "Pool budget exceeded".into(),
        });
        ctx.pool.cancel_all();
        true
    } else {
        false
    }
}

/// Handle a single pool event with retry logic.
async fn handle_pool_event(
    ctx: &mut ExecutorCtx<'_>,
    event: PoolEvent,
    state: &mut DrainState,
    subtasks: &[SubTask],
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
                ctx.pool.update_worker_cost(agent_id, cost);
            }

            if is_error {
                ctx.pool.set_worker_status(agent_id, WorkerStatus::Failed);
                let error_msg = result.clone().unwrap_or_else(|| "unknown error".into());

                if let Some(task) = find_task_for_agent(ctx.pool, agent_id, subtasks) {
                    let count = state.retry_counts.entry(task.task_id.clone()).or_insert(0);
                    if *count < ctx.config.max_retries {
                        *count += 1;
                        let attempt = *count;
                        let _ = ctx.event_tx.send(SupervisorEvent::WorkerRetrying {
                            agent_id,
                            attempt,
                            max_attempts: ctx.config.max_retries,
                        });
                        let _ = spawn_subtask(ctx, task).await;
                        return;
                    }
                }

                let _ = ctx.event_tx.send(SupervisorEvent::WorkerFailed {
                    agent_id,
                    error: error_msg.clone(),
                });
                state.results.push(WorkerResult {
                    agent_id,
                    success: false,
                    summary: error_msg,
                    cost_usd: total_cost_usd,
                });
            } else {
                ctx.pool
                    .set_worker_status(agent_id, WorkerStatus::Completed);
                let summary = result.unwrap_or_default();
                let _ = ctx.event_tx.send(SupervisorEvent::WorkerCompleted {
                    agent_id,
                    summary: summary.clone(),
                });
                state.results.push(WorkerResult {
                    agent_id,
                    success: true,
                    summary,
                    cost_usd: total_cost_usd,
                });

                // Emit canvas result flow: Worker → Supervisor(0).
                let supervisor_id = AgentId::new(0);
                ctx.emit_message_flow(agent_id, supervisor_id, EdgeMessageType::Result);
            }
        }
        Ok(AgentMessage::UsageUpdate {
            context_tokens_used,
            cost_usd,
            ..
        }) => {
            ctx.pool.update_worker_tokens(agent_id, context_tokens_used);
            if let Some(cost) = cost_usd {
                ctx.pool.update_worker_cost(agent_id, cost);
            }
            // Emit canvas token usage for the burn arc gauge.
            let budget = ctx.config.budget.per_agent_default.max_tokens;
            ctx.emit_canvas(CanvasEvent::TokenUsageUpdated {
                agent_id,
                usage: super::canvas::TokenUsage::new(context_tokens_used, budget),
            });
        }
        Ok(AgentMessage::Cancelled { reason, .. }) => {
            ctx.pool
                .set_worker_status(agent_id, WorkerStatus::Cancelled);
            let _ = ctx
                .event_tx
                .send(SupervisorEvent::WorkerCancelled { agent_id, reason });
        }
        Ok(_) => {}
        Err(e) => {
            ctx.pool.set_worker_status(agent_id, WorkerStatus::Failed);
            let _ = ctx.event_tx.send(SupervisorEvent::WorkerFailed {
                agent_id,
                error: e.to_string(),
            });
            state.results.push(WorkerResult {
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
    pool: &AgentPool,
    agent_id: AgentId,
    subtasks: &'a [SubTask],
) -> Option<&'a SubTask> {
    let snapshot = pool.status_snapshot();
    let worker = snapshot.iter().find(|s| s.id == agent_id)?;
    let task_id = match &worker.role {
        super::topology::AgentRole::Worker { task_id } => task_id,
        _ => return None,
    };
    subtasks.iter().find(|t| &t.task_id == task_id)
}

// ─── ParallelTddExecutor ──────────────────────────────────────────────────

/// All agents start simultaneously with partitioned subtasks.
///
/// The supervisor monitors file conflicts and serializes overlapping edits.
/// When all workers complete, the supervisor runs quality gates on the merged
/// result. Failed agents are retried up to the budget limit.
struct ParallelTddExecutor;

impl PatternExecutor for ParallelTddExecutor {
    async fn execute(
        &self,
        ctx: &mut ExecutorCtx<'_>,
        subtasks: Vec<SubTask>,
    ) -> Result<Vec<WorkerResult>> {
        // Phase 1: Spawn all subtasks simultaneously
        for task in &subtasks {
            spawn_subtask(ctx, task).await?;
        }

        let mut state = DrainState::new();

        // Phase 2: Monitor until all done, checking conflicts as we go
        while !ctx.pool.is_all_done() {
            if budget_exceeded(ctx) {
                break;
            }

            let event = match ctx.pool.next_event().await {
                Some(e) => e,
                None => break,
            };

            // Emit conflict events before handling
            for conflict in ctx.conflicts.detect_conflicts() {
                let _ = ctx.event_tx.send(SupervisorEvent::ConflictDetected {
                    file: conflict.file.clone(),
                    agents: conflict.agents.clone(),
                });
                // Emit canvas conflict event for pulsing halos and jagged edges.
                ctx.emit_canvas(CanvasEvent::ConflictDetected {
                    file: conflict.file,
                    agents: conflict.agents,
                });
            }

            handle_pool_event(ctx, event, &mut state, &subtasks).await;
        }

        // Phase 3: Run quality gates on merged result
        let gate_results = run_quality_gates(ctx).await;
        if let Ok(gate) = gate_results {
            state.results.extend(gate);
        }

        Ok(state.results)
    }
}

/// Spawn a quality-gate worker that runs check + test + lint on the merged output.
async fn run_quality_gates(ctx: &mut ExecutorCtx<'_>) -> Result<Vec<WorkerResult>> {
    let gate_task = SubTask {
        task_id: "quality-gate".into(),
        description: "Run quality gates on merged output".into(),
        prompt: "Run all quality gates: cargo check --all-targets && cargo nextest run && \
                 cargo clippy -- -D warnings. Report pass/fail for each gate."
            .into(),
        scope: vec![ctx.config.cwd.clone()],
        strategy: super::partition::PartitionStrategy::SplitByPhase,
        archetype_id: None,
    };

    let id = spawn_subtask(ctx, &gate_task).await?;

    let mut state = DrainState::new();
    drain_for_agents(ctx, &[id], &mut state, &[gate_task]).await;

    let snapshot = ctx.pool.status_snapshot();
    let gate_worker = snapshot.iter().find(|s| s.id == id);

    Ok(match gate_worker {
        Some(w) if w.status == WorkerStatus::Completed => vec![WorkerResult {
            agent_id: id,
            success: true,
            summary: "Quality gates passed".into(),
            cost_usd: Some(w.cost_usd),
        }],
        Some(w) => vec![WorkerResult {
            agent_id: id,
            success: false,
            summary: "Quality gates failed".into(),
            cost_usd: Some(w.cost_usd),
        }],
        None => Vec::new(),
    })
}

// ─── ResearchThenImplementExecutor ─────────────────────────────────────────

/// Researcher agent runs first (read-only, web search + codebase exploration).
///
/// On completion, its findings are injected as context into the Coder agent's
/// system prompt. The Coder then implements with full tool access.
/// Sequential handoff with context bridging.
struct ResearchThenImplementExecutor;

impl PatternExecutor for ResearchThenImplementExecutor {
    async fn execute(
        &self,
        ctx: &mut ExecutorCtx<'_>,
        subtasks: Vec<SubTask>,
    ) -> Result<Vec<WorkerResult>> {
        // Separate research tasks from implementation tasks
        let (research_tasks, impl_tasks): (Vec<_>, Vec<_>) = subtasks
            .into_iter()
            .partition(|t| is_research_task(&t.task_id, &t.description));

        let mut state = DrainState::new();

        // Phase 1: Run research tasks
        let mut research_ids = Vec::new();
        for task in &research_tasks {
            let id = spawn_subtask(ctx, task).await?;
            research_ids.push(id);
        }

        if !research_ids.is_empty() {
            drain_for_agents(ctx, &research_ids, &mut state, &research_tasks).await;

            // Publish research findings to shared context
            for result in &state.results {
                if result.success {
                    ctx.context.publish(
                        result.agent_id,
                        "research_findings",
                        result.summary.clone(),
                    );

                    // Emit canvas context-update flow: Researcher → Supervisor.
                    let supervisor_id = AgentId::new(0);
                    ctx.emit_message_flow(
                        result.agent_id,
                        supervisor_id,
                        EdgeMessageType::ContextUpdate,
                    );
                }
            }
        }

        // Phase 2: Build context prefix from research findings
        let context_prefix = build_context_prefix(ctx.context);

        // Phase 3: Run implementation tasks with research context injected
        let augmented_tasks: Vec<SubTask> = impl_tasks
            .into_iter()
            .map(|mut task| {
                if !context_prefix.is_empty() {
                    task.prompt = format!(
                        "## Research Findings\n\n{context_prefix}\n\n---\n\n## Task\n\n{}",
                        task.prompt
                    );
                }
                task
            })
            .collect();

        for task in &augmented_tasks {
            spawn_subtask(ctx, task).await?;
        }

        drain_until_done(ctx, &mut state, &augmented_tasks).await;

        Ok(state.results)
    }
}

/// Check if a subtask is a research task based on ID or description.
fn is_research_task(task_id: &str, description: &str) -> bool {
    let id_lower = task_id.to_lowercase();
    let desc_lower = description.to_lowercase();
    id_lower.contains("research")
        || desc_lower.contains("research")
        || desc_lower.contains("investigate")
        || desc_lower.contains("explore")
        || desc_lower.contains("analyze")
}

/// Build a context prefix string from all research findings in the shared context.
fn build_context_prefix(context: &SharedContext) -> String {
    let findings = context.query("research_findings");
    if findings.is_empty() {
        return String::new();
    }

    findings
        .iter()
        .enumerate()
        .map(|(i, f)| format!("### Finding {}\n\n{}", i + 1, f.value))
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ─── ReviewPairExecutor ────────────────────────────────────────────────────

/// Coder and Reviewer run in alternating rounds.
///
/// The Coder implements, then pauses. The Reviewer reads the diff and outputs
/// structured critique. The critique is injected into the Coder's next prompt
/// for revisions. Cycles until the Reviewer approves or the iteration limit
/// is hit.
struct ReviewPairExecutor {
    /// Maximum number of review rounds before accepting.
    max_rounds: u32,
}

impl Default for ReviewPairExecutor {
    fn default() -> Self {
        Self { max_rounds: 3 }
    }
}

impl PatternExecutor for ReviewPairExecutor {
    async fn execute(
        &self,
        ctx: &mut ExecutorCtx<'_>,
        subtasks: Vec<SubTask>,
    ) -> Result<Vec<WorkerResult>> {
        // Separate coder and reviewer tasks
        let (coder_tasks, reviewer_template): (Vec<_>, Option<SubTask>) = {
            let mut coders = Vec::new();
            let mut reviewer = None;
            for task in subtasks {
                if is_reviewer_task(&task.task_id, &task.description) {
                    reviewer = Some(task);
                } else {
                    coders.push(task);
                }
            }
            (coders, reviewer)
        };

        let mut state = DrainState::new();
        let mut round = 0;
        let mut last_critique = String::new();

        loop {
            round += 1;
            if round > self.max_rounds {
                break;
            }

            // Phase 1: Run coder with any previous critique injected
            let round_tasks: Vec<SubTask> = coder_tasks
                .iter()
                .map(|task| {
                    let mut augmented = task.clone();
                    augmented.task_id = format!("{}-round-{round}", task.task_id);
                    if !last_critique.is_empty() {
                        augmented.prompt = format!(
                            "## Previous Review Feedback\n\n{last_critique}\n\n---\n\n## Task\n\n{}",
                            augmented.prompt
                        );
                    }
                    augmented
                })
                .collect();

            let mut coder_ids = Vec::new();
            for task in &round_tasks {
                let id = spawn_subtask(ctx, task).await?;
                coder_ids.push(id);
            }

            let results_before = state.results.len();
            drain_for_agents(ctx, &coder_ids, &mut state, &round_tasks).await;
            let coder_results: Vec<_> = state.results[results_before..].to_vec();

            // If coder failed, stop the loop
            let coder_success = coder_results.iter().all(|r| r.success);
            if !coder_success {
                break;
            }

            // Publish coder output to context for reviewer
            for result in &coder_results {
                ctx.context
                    .publish(result.agent_id, "coder_output", result.summary.clone());
            }

            // Phase 2: Run reviewer with coder's output
            let coder_summary: String = coder_results
                .iter()
                .map(|r| r.summary.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");

            let review_task = build_review_task(
                round,
                &coder_summary,
                reviewer_template.as_ref(),
                &ctx.config.cwd,
            );

            let reviewer_id = spawn_subtask(ctx, &review_task).await?;

            let results_before = state.results.len();
            drain_for_agents(ctx, &[reviewer_id], &mut state, &[review_task]).await;

            let review_summary = state
                .results
                .get(results_before)
                .map(|r| r.summary.clone())
                .unwrap_or_default();

            if is_approved(&review_summary) {
                break;
            }

            last_critique = review_summary;
        }

        Ok(state.results)
    }
}

/// Build the review task for a given round.
fn build_review_task(
    round: u32,
    coder_summary: &str,
    template: Option<&SubTask>,
    cwd: &std::path::Path,
) -> SubTask {
    if let Some(template) = template {
        SubTask {
            task_id: format!("{}-round-{round}", template.task_id),
            description: template.description.clone(),
            prompt: format!(
                "## Code Changes (Round {round})\n\n{coder_summary}\n\n---\n\n\
                 ## Review Instructions\n\n{}\n\n\
                 Respond with either:\n\
                 - APPROVED: <brief summary> — if the code is ready\n\
                 - REVISIONS NEEDED: <detailed critique> — if changes are needed",
                template.prompt
            ),
            scope: template.scope.clone(),
            strategy: template.strategy.clone(),
            archetype_id: template.archetype_id.clone(),
        }
    } else {
        SubTask {
            task_id: format!("reviewer-round-{round}"),
            description: "Review the implementation".into(),
            prompt: format!(
                "## Code Changes (Round {round})\n\n{coder_summary}\n\n---\n\n\
                 Review the code changes above for correctness, style, and completeness.\n\n\
                 Respond with either:\n\
                 - APPROVED: <brief summary> — if the code is ready\n\
                 - REVISIONS NEEDED: <detailed critique> — if changes are needed"
            ),
            scope: vec![cwd.to_path_buf()],
            strategy: super::partition::PartitionStrategy::SplitByPhase,
            archetype_id: None,
        }
    }
}

/// Check if a subtask is a reviewer task.
fn is_reviewer_task(task_id: &str, description: &str) -> bool {
    let id_lower = task_id.to_lowercase();
    let desc_lower = description.to_lowercase();
    id_lower.contains("review") || desc_lower.contains("review") || desc_lower.contains("critique")
}

/// Check if a review response indicates approval.
fn is_approved(summary: &str) -> bool {
    let upper = summary.to_uppercase();
    upper.contains("APPROVED") && !upper.contains("NOT APPROVED")
}

#[cfg(test)]
#[path = "pattern.tests.rs"]
mod tests;
