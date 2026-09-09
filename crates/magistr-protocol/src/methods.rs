//! Protocol method definitions — typed request params and result types.
//!
//! Each domain (session, workflow, roadmap, cost, project, system) defines
//! its methods as constants plus strongly-typed `Params` and `Result` structs.
//! The server uses these to validate and dispatch JSON-RPC requests.

use magistr_engine::session::SessionEvent;
use magistr_engine::{
    BatchStatus, BudgetConfig, FleetStatus, IterationState, PolicyBundle, ProjectContext,
    ProjectDep, RoadmapProgress, SessionCommand, SessionCost, SessionSnapshot, SessionStatus,
    WorkflowCommand, WorkflowConfig, WorkflowPhase,
};
use serde::{Deserialize, Serialize};

// ── Method name constants ───────────────────────────────────────────────────

/// All valid method names, organized by domain.
pub mod method {
    // Session
    pub const SESSION_CREATE: &str = "session.create";
    pub const SESSION_STATUS: &str = "session.status";
    pub const SESSION_COMMAND: &str = "session.command";
    pub const SESSION_LIST: &str = "session.list";

    // Workflow
    pub const WORKFLOW_PHASE: &str = "workflow.phase";
    pub const WORKFLOW_SET_PHASE: &str = "workflow.setPhase";
    pub const WORKFLOW_COMMAND: &str = "workflow.command";
    pub const WORKFLOW_ITERATION: &str = "workflow.iteration";

    // Roadmap
    pub const ROADMAP_STATUS: &str = "roadmap.status";
    pub const ROADMAP_TASKS: &str = "roadmap.tasks";
    pub const ROADMAP_CHECK: &str = "roadmap.check";
    pub const ROADMAP_ADD_TASK: &str = "roadmap.addTask";

    // Cost
    pub const COST_STATUS: &str = "cost.status";
    pub const COST_EXTEND_BUDGET: &str = "cost.extendBudget";

    // Project
    pub const PROJECT_CONTEXT: &str = "project.context";
    pub const PROJECT_REFRESH: &str = "project.refresh";

    // System
    pub const SYSTEM_VERSION: &str = "system.version";
    pub const SYSTEM_CAPABILITIES: &str = "system.capabilities";
    pub const SYSTEM_SHUTDOWN: &str = "system.shutdown";
    pub const SYSTEM_HEALTH: &str = "system.health";
    pub const SYSTEM_METRICS: &str = "system.metrics";
    pub const SYSTEM_REGISTER_PROJECT: &str = "system.registerProject";
    pub const SYSTEM_UNREGISTER_PROJECT: &str = "system.unregisterProject";
    pub const SYSTEM_LIST_PROJECTS: &str = "system.listProjects";

    // Fleet
    pub const FLEET_REGISTER: &str = "fleet.register";
    pub const FLEET_HEARTBEAT: &str = "fleet.heartbeat";
    pub const FLEET_PUBLISH_POLICY: &str = "fleet.publishPolicy";
    pub const FLEET_NODES: &str = "fleet.nodes";
    pub const FLEET_STATUS: &str = "fleet.status";
    /// Cordon a node off, or let it take work again, and hand back
    /// whatever it was holding. See `fleet.drain` in the dispatcher.
    pub const FLEET_DRAIN: &str = "fleet.drain";

    // Work — remote agent execution over the fleet
    pub const WORK_SUBMIT: &str = "work.submit";
    pub const WORK_CLAIM: &str = "work.claim";
    pub const WORK_REPORT: &str = "work.report";
    pub const WORK_COMPLETE: &str = "work.complete";
    pub const WORK_EVENTS: &str = "work.events";
    pub const WORK_STATUS: &str = "work.status";
    /// A node writing down the session state of the job it holds, so a
    /// hand-off resumes rather than restarts.
    pub const WORK_CHECKPOINT: &str = "work.checkpoint";

    // ── Multi-project batches ────────────────────────────────
    pub const BATCH_SUBMIT: &str = "batch.submit";
    pub const BATCH_STATUS: &str = "batch.status";

    /// All valid method names for validation.
    pub const ALL: &[&str] = &[
        SESSION_CREATE,
        SESSION_STATUS,
        SESSION_COMMAND,
        SESSION_LIST,
        WORKFLOW_PHASE,
        WORKFLOW_SET_PHASE,
        WORKFLOW_COMMAND,
        WORKFLOW_ITERATION,
        ROADMAP_STATUS,
        ROADMAP_TASKS,
        ROADMAP_CHECK,
        ROADMAP_ADD_TASK,
        COST_STATUS,
        COST_EXTEND_BUDGET,
        PROJECT_CONTEXT,
        PROJECT_REFRESH,
        SYSTEM_VERSION,
        SYSTEM_CAPABILITIES,
        SYSTEM_SHUTDOWN,
        SYSTEM_HEALTH,
        SYSTEM_METRICS,
        SYSTEM_REGISTER_PROJECT,
        SYSTEM_UNREGISTER_PROJECT,
        SYSTEM_LIST_PROJECTS,
        FLEET_REGISTER,
        FLEET_HEARTBEAT,
        FLEET_PUBLISH_POLICY,
        FLEET_NODES,
        FLEET_STATUS,
        FLEET_DRAIN,
        WORK_SUBMIT,
        WORK_CLAIM,
        WORK_REPORT,
        WORK_COMPLETE,
        WORK_EVENTS,
        WORK_STATUS,
        WORK_CHECKPOINT,
        BATCH_SUBMIT,
        BATCH_STATUS,
    ];

    /// Check whether a method name is valid.
    pub fn is_valid(method: &str) -> bool {
        ALL.contains(&method)
    }
}

// ── Session methods ─────────────────────────────────────────────────────────

/// Parameters for `session.create`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCreateParams {
    /// Optional session ID (auto-generated if omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Workflow configuration for this session.
    pub workflow_config: WorkflowConfig,
    /// Budget configuration for this session.
    #[serde(default)]
    pub budget_config: BudgetConfig,
}

/// Result for `session.create`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCreateResult {
    /// The assigned session ID.
    pub session_id: String,
    /// Initial session status.
    pub status: SessionStatus,
    /// Initial workflow phase.
    pub phase: WorkflowPhase,
}

/// Parameters for `session.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatusParams {
    /// Session ID to query.
    pub session_id: String,
}

/// Result for `session.status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStatusResult {
    /// Session ID.
    pub session_id: String,
    /// Current lifecycle status.
    pub status: SessionStatus,
    /// Current workflow phase.
    pub phase: WorkflowPhase,
    /// Accumulated cost and usage.
    pub cost: SessionCost,
}

/// Parameters for `session.command`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCommandParams {
    /// Target session ID.
    pub session_id: String,
    /// The command to execute.
    pub command: SessionCommand,
}

/// Result for `session.command`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCommandResult {
    /// Whether the command was accepted.
    pub accepted: bool,
    /// New session status after the command.
    pub status: SessionStatus,
}

/// Result for `session.list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionListResult {
    /// Active sessions.
    pub sessions: Vec<SessionSummary>,
}

/// Summary of a session for listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session ID.
    pub session_id: String,
    /// Current status.
    pub status: SessionStatus,
    /// Current phase.
    pub phase: WorkflowPhase,
    /// Total cost in USD.
    pub total_cost_usd: f64,
}

// ── Workflow methods ────────────────────────────────────────────────────────

/// Parameters for `workflow.phase` (query current phase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPhaseParams {
    /// Session ID.
    pub session_id: String,
}

/// Result for `workflow.phase`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowPhaseResult {
    /// Current phase.
    pub phase: WorkflowPhase,
    /// Phase step index (0-based).
    pub step_index: usize,
    /// Human-readable label.
    pub label: String,
}

/// Parameters for `workflow.setPhase`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSetPhaseParams {
    /// Session ID.
    pub session_id: String,
    /// Target phase.
    pub phase: WorkflowPhase,
}

/// Result for `workflow.setPhase`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSetPhaseResult {
    /// Previous phase.
    pub from: WorkflowPhase,
    /// New phase.
    pub to: WorkflowPhase,
}

/// Parameters for `workflow.command`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCommandParams {
    /// Session ID.
    pub session_id: String,
    /// The workflow command to execute.
    pub command: WorkflowCommand,
}

/// Result for `workflow.command`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowCommandResult {
    /// Whether the command was accepted.
    pub accepted: bool,
    /// Current phase after the command.
    pub phase: WorkflowPhase,
    /// Whether the loop is paused.
    pub paused: bool,
}

/// Parameters for `workflow.iteration` (query iteration state).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowIterationParams {
    /// Session ID.
    pub session_id: String,
}

/// Result for `workflow.iteration`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowIterationResult {
    /// Current iteration state.
    pub iteration: IterationState,
    /// Whether the loop should stop.
    pub should_stop: bool,
}

// ── Roadmap methods ─────────────────────────────────────────────────────────

/// Result for `roadmap.status` (no params needed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapStatusResult {
    /// Current progress.
    pub progress: RoadmapProgress,
    /// Completion percentage (0–100).
    pub completion_pct: u32,
    /// Whether the spec is fully complete.
    pub is_complete: bool,
}

/// Parameters for `roadmap.tasks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapTasksParams {
    /// Filter to a specific phase (e.g., "E1"). Omit for all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_id: Option<String>,
    /// Whether to only return unchecked tasks.
    #[serde(default)]
    pub unchecked_only: bool,
}

/// A task in the roadmap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapTask {
    /// Task ID (e.g., "#16").
    pub id: String,
    /// Task description.
    pub text: String,
    /// Whether the task is checked off.
    pub checked: bool,
    /// Phase this task belongs to.
    pub phase_id: String,
}

/// Result for `roadmap.tasks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapTasksResult {
    /// Matching tasks.
    pub tasks: Vec<RoadmapTask>,
}

/// Parameters for `roadmap.check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapCheckParams {
    /// Task ID to check off.
    pub task_id: String,
}

/// Result for `roadmap.check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapCheckResult {
    /// Whether the task was toggled (false if already checked).
    pub toggled: bool,
    /// Updated progress.
    pub progress: RoadmapProgress,
}

/// Parameters for `roadmap.addTask`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapAddTaskParams {
    /// Phase to add the task to.
    pub phase_id: String,
    /// Task description.
    pub text: String,
}

/// Result for `roadmap.addTask`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapAddTaskResult {
    /// Assigned task ID.
    pub task_id: String,
    /// Updated progress.
    pub progress: RoadmapProgress,
}

// ── Cost methods ────────────────────────────────────────────────────────────

/// Parameters for `cost.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostStatusParams {
    /// Session ID.
    pub session_id: String,
}

/// Result for `cost.status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostStatusResult {
    /// Accumulated cost.
    pub cost: SessionCost,
    /// Budget configuration.
    pub budget: BudgetConfig,
    /// Whether the budget has been exceeded.
    pub is_exceeded: bool,
}

/// Parameters for `cost.extendBudget`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostExtendBudgetParams {
    /// Session ID.
    pub session_id: String,
    /// New budget limit in USD.
    pub new_limit: f64,
}

/// Result for `cost.extendBudget`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostExtendBudgetResult {
    /// Previous limit (None if no limit was set).
    pub old_limit: Option<f64>,
    /// New limit.
    pub new_limit: f64,
}

// ── Project methods ─────────────────────────────────────────────────────────

/// Result for `project.context` (no params needed).
pub type ProjectContextResult = ProjectContext;

/// Result for `project.refresh` (no params needed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRefreshResult {
    /// Whether the context was actually changed.
    pub changed: bool,
    /// The refreshed context.
    pub context: ProjectContext,
}

// ── System methods ──────────────────────────────────────────────────────────

/// Result for `system.version` (no params needed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemVersionResult {
    /// Protocol version (semver).
    pub protocol_version: String,
    /// Server version (semver).
    pub server_version: String,
}

/// Result for `system.capabilities` (no params needed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemCapabilitiesResult {
    /// Supported domains.
    pub domains: Vec<String>,
    /// Available methods.
    pub methods: Vec<String>,
    /// Supported notification types.
    pub notifications: Vec<String>,
    /// Supported transports.
    pub transports: Vec<String>,
}

/// Result for `system.shutdown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemShutdownResult {
    /// Whether shutdown was initiated.
    pub shutting_down: bool,
}

/// Result for `system.health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemHealthResult {
    /// Server status (`ok` or `shutting_down`).
    pub status: String,
    /// Server uptime in seconds.
    pub uptime_secs: u64,
    /// Number of connected clients.
    pub client_count: usize,
    /// Active engine session ID.
    pub session_id: String,
    /// Server version.
    pub version: String,
}

/// Result for `system.metrics`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemMetricsResult {
    /// Total sessions created since server start.
    pub sessions_total: u64,
    /// Currently active sessions.
    pub sessions_active: u64,
    /// Total quality gates executed.
    pub gates_run: u64,
    /// Quality gates that passed.
    pub gates_passed: u64,
    /// Quality gates that failed.
    pub gates_failed: u64,
    /// Accumulated cost in millicents (1/1000 of a cent).
    pub cost_total_millicents: u64,
    /// Total JSON-RPC requests processed.
    pub requests_total: u64,
    /// Server uptime in seconds.
    pub uptime_secs: u64,
    /// Prometheus text exposition format.
    pub prometheus: String,
}

/// Parameters for `system.registerProject`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemRegisterProjectParams {
    /// Unique project identifier.
    pub project_id: String,
    /// Absolute path to the project directory.
    pub project_dir: String,
}

/// Result for `system.registerProject`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemRegisterProjectResult {
    /// The registered project ID.
    pub project_id: String,
    /// The engine session ID assigned to this project.
    pub session_id: String,
}

/// Parameters for `system.unregisterProject`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemUnregisterProjectParams {
    /// Project ID to unregister.
    pub project_id: String,
}

/// Result for `system.unregisterProject`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemUnregisterProjectResult {
    /// Whether the project was found and removed.
    pub removed: bool,
}

/// Result for `system.listProjects`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemListProjectsResult {
    /// Registered project IDs.
    pub projects: Vec<String>,
    /// Number of registered projects.
    pub count: usize,
}

// ── Fleet methods ───────────────────────────────────────────────────────────

/// Parameters for `fleet.register` — a node joining the fleet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetRegisterParams {
    /// Stable node id, kept across restarts.
    pub node_id: String,
    /// Host the node runs on.
    pub hostname: String,
    /// The magistr version the node runs.
    pub version: String,
    /// Project ids this node serves. Empty means general-purpose.
    #[serde(default)]
    pub projects: Vec<String>,
    /// How many units of work the node will hold at once. Unstated
    /// means one — the conservative reading, so a node that can do more
    /// has to say so.
    #[serde(default)]
    pub capacity: Option<u32>,
}

/// Result for `fleet.register` — the lease terms and the bundle to apply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetRegisterResult {
    /// The registered node id.
    pub node_id: String,
    /// How long a lease is good for; renew inside this window.
    pub lease_ttl_secs: i64,
    /// The policy bundle the node must apply.
    pub bundle: PolicyBundle,
}

/// Parameters for `fleet.heartbeat` — one lease renewal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetHeartbeatParams {
    /// The node renewing its lease.
    pub node_id: String,
    /// The bundle generation the node is running.
    #[serde(default)]
    pub applied_generation: u64,
    /// The bundle digest the node is running.
    #[serde(default)]
    pub applied_digest: String,
}

/// Result for `fleet.heartbeat`.
///
/// `bundle` is present only when the node is behind — a converged fleet
/// heartbeats without moving policy over the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetHeartbeatResult {
    /// Whether the node is running the current bundle.
    pub current: bool,
    /// The bundle to apply, when it is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle: Option<PolicyBundle>,
}

/// Parameters for `fleet.publishPolicy` — a new generation of guardrails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetPublishPolicyParams {
    /// The bundle to publish. Its generation must be strictly newer
    /// than the published one, and its digest must match its contents.
    pub bundle: PolicyBundle,
}

/// Result for `fleet.publishPolicy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetPublishPolicyResult {
    /// The generation now published.
    pub generation: u64,
    /// Its digest.
    pub digest: String,
}

/// One row of `fleet.nodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodeRow {
    /// The node id.
    pub node_id: String,
    /// Host the node runs on.
    pub hostname: String,
    /// The magistr version the node runs.
    pub version: String,
    /// Project ids this node serves.
    pub projects: Vec<String>,
    /// Health derived from the node's lease: ready, stale, unreachable.
    pub health: String,
    /// Seconds since the node last renewed its lease.
    pub lease_age_secs: i64,
    /// The bundle generation the node reports running.
    pub applied_generation: u64,
    /// Whether that is the current bundle.
    pub current: bool,
    /// How many units of work the node will hold at once.
    pub capacity: u32,
}

/// Result for `fleet.nodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNodesResult {
    /// Every known node, in id order.
    pub nodes: Vec<FleetNodeRow>,
    /// How many there are.
    pub count: usize,
}

/// Result for `fleet.status` — the whole-fleet rollup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetStatusResult {
    /// Health and convergence counts.
    #[serde(flatten)]
    pub status: FleetStatus,
    /// Whether every node is reachable and on the current bundle.
    pub converged: bool,
    /// The fleet's lease TTL.
    pub lease_ttl_secs: i64,
}

/// Parameters for `fleet.drain` — take a node out of service, or put it
/// back.
///
/// One method rather than a `cordon` and a `drain`, because the two are
/// only safe in one order: cordoning without evicting leaves a node
/// quiesced (which `evict: false` asks for), while evicting without
/// cordoning would place the freed work straight back on the machine it
/// was freed from. Doing both here means a client cannot get the order
/// wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetDrainParams {
    /// The node to take out of service.
    pub node_id: String,
    /// Whether the node is being cordoned (the default) or released
    /// back into service.
    #[serde(default = "crate::methods::default_true")]
    pub draining: bool,
    /// Whether to hand back the work the node is already holding.
    /// Ignored when `draining` is false — a node coming back into
    /// service has nothing to evict.
    #[serde(default = "crate::methods::default_true")]
    pub evict: bool,
}

/// Result for `fleet.drain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetDrainResult {
    /// The node.
    pub node_id: String,
    /// Whether it is now cordoned off.
    pub draining: bool,
    /// The jobs handed back to the queue, in submission order. Each is
    /// re-queued carrying whatever state its holder last checkpointed,
    /// so another node resumes it rather than starting it again.
    #[serde(default)]
    pub handed_off: Vec<String>,
}

/// What a serde field default of `true` looks like.
pub(crate) fn default_true() -> bool {
    true
}

// ── Work methods ────────────────────────────────────────────────────────────

/// Parameters for `work.submit` — a unit of work for the fleet to run.
///
/// The id comes from the submitter so that a client retrying a request
/// whose answer it never saw does not create a second job: a repeat is
/// refused rather than duplicated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSubmitParams {
    /// Stable job id, chosen by the submitter.
    pub job_id: String,
    /// The project this work runs against.
    pub project: String,
    /// The prompt the remote agent is asked to run.
    pub prompt: String,
    /// The client asking for it, for attributing the event stream.
    #[serde(default)]
    pub requested_by: String,
}

/// Result for `work.submit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSubmitResult {
    /// The accepted job id.
    pub job_id: String,
    /// Its state — `queued` for a job nothing has taken yet.
    pub state: String,
}

/// Parameters for `work.claim` — a node asking the control plane for work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkClaimParams {
    /// The node asking.
    pub node_id: String,
}

/// What a node was given to run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkAssignment {
    /// The job to run.
    pub job_id: String,
    /// Its project.
    pub project: String,
    /// The prompt to run.
    pub prompt: String,
    /// The attempt number this claim minted. Every later message about
    /// this job — an event, a completion — must carry it back, or it is
    /// refused as belonging to a holder the job has moved past.
    pub attempt: u32,
    /// How long the claim is good for without being re-taken.
    pub claim_ttl_secs: i64,
    /// The state the job's previous holder last checkpointed, when there
    /// was one. Present on a job that has been reclaimed or drained, so
    /// this attempt resumes rather than starting the work again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_from: Option<SessionSnapshot>,
}

/// Result for `work.claim` — an assignment, or nothing to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkClaimResult {
    /// The work, when there was any for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<WorkAssignment>,
    /// Jobs this request's sweep reclaimed from nodes that had gone.
    #[serde(default)]
    pub reclaimed: Vec<String>,
}

/// Parameters for `work.report` — one session event from a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkReportParams {
    /// The job the event belongs to.
    pub job_id: String,
    /// The node reporting it.
    pub node_id: String,
    /// The attempt the node holds.
    pub attempt: u32,
    /// The event's position in this attempt's stream, from zero.
    pub seq: u64,
    /// The event itself.
    pub event: SessionEvent,
}

/// Result for `work.report`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkReportResult {
    /// How many events became readable — zero for a repeat, more than
    /// one when this event closed a gap ahead of others.
    pub delivered: usize,
    /// The sequence number the relay is waiting on next.
    pub next_seq: u64,
}

/// Parameters for `work.complete` — the outcome a node reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCompleteParams {
    /// The job that finished.
    pub job_id: String,
    /// The node that ran it.
    pub node_id: String,
    /// The attempt it holds.
    pub attempt: u32,
    /// Whether the work succeeded.
    pub succeeded: bool,
}

/// Result for `work.complete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCompleteResult {
    /// The job id.
    pub job_id: String,
    /// Its settled state — `completed` or `failed`.
    pub state: String,
}

/// Parameters for `work.checkpoint` — a node writing down the session
/// state of the job it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkCheckpointParams {
    /// The job being checkpointed.
    pub job_id: String,
    /// The node holding it.
    pub node_id: String,
    /// The attempt it holds. Fenced exactly like `work.report`, so a
    /// node that lost the job cannot overwrite the new holder's state.
    pub attempt: u32,
    /// The session as it stands. A checkpoint is a level, not an
    /// increment: the newest replaces whatever was there.
    pub snapshot: SessionSnapshot,
}

/// Result for `work.checkpoint`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCheckpointResult {
    /// The job.
    pub job_id: String,
    /// The attempt the checkpoint was accepted for.
    pub attempt: u32,
}

/// Parameters for `work.events` — a client reading a job's stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkEventsParams {
    /// The job to read.
    pub job_id: String,
    /// How far the client has already read. The log is append-only
    /// across attempts, so a cursor never has to be rewound.
    #[serde(default)]
    pub cursor: usize,
}

/// Result for `work.events` — everything after the cursor, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkEventsResult {
    /// The job id.
    pub job_id: String,
    /// The events, oldest first.
    pub events: Vec<SessionEvent>,
    /// The cursor to send next time.
    pub cursor: usize,
}

/// One row of `work.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkJobRow {
    /// The job id.
    pub job_id: String,
    /// Its project.
    pub project: String,
    /// Its state: queued, claimed, completed, failed.
    pub state: String,
    /// The node holding it, while one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// How many times it has been handed to a node.
    pub attempt: u32,
    /// How many events it has produced for the client to read.
    pub events: usize,
}

/// Result for `work.status` — the queue rollup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkStatusResult {
    /// Every job, in submission order.
    pub jobs: Vec<WorkJobRow>,
    /// How many are waiting for a node.
    pub queued: usize,
    /// How many a node is holding.
    pub claimed: usize,
    /// How many finished successfully.
    pub completed: usize,
    /// How many finished unsuccessfully.
    pub failed: usize,
    /// How long a claim is good for.
    pub claim_ttl_secs: i64,
    /// Jobs this request's sweep reclaimed from nodes that had gone.
    #[serde(default)]
    pub reclaimed: Vec<String>,
}

/// Parameters for `batch.submit` — one prompt across many projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchSubmitParams {
    /// The batch id, chosen by the submitter. Job ids are derived from
    /// it as `<batch>:<project>`, so a retried submission collides on
    /// the queue rather than running the batch twice.
    pub batch_id: String,
    /// The prompt every project in the batch is given.
    pub prompt: String,
    /// The projects, their in-batch dependencies and their working
    /// trees.
    pub projects: Vec<ProjectDep>,
    /// The client the batch's streams belong to.
    #[serde(default)]
    pub requested_by: String,
}

/// Result for `batch.submit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchSubmitResult {
    /// The accepted batch id.
    pub batch_id: String,
    /// The projects in the order their dependencies allow.
    pub order: Vec<String>,
    /// The projects released by this call — the first wave.
    pub released: Vec<String>,
    /// The rollup as it stands after the release.
    pub status: BatchStatus,
}

/// Parameters for `batch.status` — reading one batch's rollup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchStatusParams {
    /// The batch to read.
    pub batch_id: String,
}

/// Result for `batch.status` — the rollup, plus whatever the sweep
/// released on the way to answering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchStatusResult {
    /// The rollup: one answer for the whole run.
    pub status: BatchStatus,
    /// The projects this request's sweep released.
    #[serde(default)]
    pub released: Vec<String>,
    /// Jobs this request's sweep reclaimed from nodes that had gone.
    #[serde(default)]
    pub reclaimed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use magistr_engine::BatchRow;

    // ── Method name validation ──────────────────────────────────────────

    #[test]
    fn all_methods_are_valid() {
        for &m in method::ALL {
            assert!(method::is_valid(m), "method should be valid: {m}");
        }
    }

    #[test]
    fn unknown_method_is_invalid() {
        assert!(!method::is_valid("foo.bar"));
        assert!(!method::is_valid("session.destroy"));
    }

    #[test]
    fn method_count() {
        // The table includes control-plane, work-queue, and migration methods.
        assert_eq!(method::ALL.len(), 39);
    }

    // ── Session params/results ──────────────────────────────────────────

    #[test]
    fn session_create_params_roundtrip() {
        let params = SessionCreateParams {
            session_id: Some("test-123".into()),
            workflow_config: WorkflowConfig {
                project_dir: "/tmp/proj".into(),
                spec_path: "ROADMAP.md".into(),
                quality_gate: None,
                max_iterations: 10,
                lifecycle_hooks: vec![],
            },
            budget_config: BudgetConfig::with_limit(5.0),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: SessionCreateParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, parsed);
    }

    #[test]
    fn session_create_result_roundtrip() {
        let result = SessionCreateResult {
            session_id: "abc".into(),
            status: SessionStatus::Created,
            phase: WorkflowPhase::Researching,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SessionCreateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    #[test]
    fn session_command_params_roundtrip() {
        let params = SessionCommandParams {
            session_id: "sess-1".into(),
            command: SessionCommand::Cancel,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: SessionCommandParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, parsed);
    }

    #[test]
    fn session_list_result_roundtrip() {
        let result = SessionListResult {
            sessions: vec![SessionSummary {
                session_id: "s1".into(),
                status: SessionStatus::InProgress,
                phase: WorkflowPhase::Executing,
                total_cost_usd: 1.23,
            }],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SessionListResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    // ── Workflow params/results ─────────────────────────────────────────

    #[test]
    fn workflow_phase_result_roundtrip() {
        let result = WorkflowPhaseResult {
            phase: WorkflowPhase::Planning,
            step_index: 1,
            label: "Planning".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: WorkflowPhaseResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    #[test]
    fn workflow_command_params_roundtrip() {
        let params = WorkflowCommandParams {
            session_id: "s1".into(),
            command: WorkflowCommand::Pause,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: WorkflowCommandParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, parsed);
    }

    #[test]
    fn workflow_set_phase_roundtrip() {
        let params = WorkflowSetPhaseParams {
            session_id: "s1".into(),
            phase: WorkflowPhase::Verifying,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: WorkflowSetPhaseParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, parsed);
    }

    // ── Roadmap params/results ──────────────────────────────────────────

    #[test]
    fn roadmap_tasks_params_defaults() {
        let json = r#"{"unchecked_only": false}"#;
        let parsed: RoadmapTasksParams = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.phase_id, None);
        assert!(!parsed.unchecked_only);
    }

    #[test]
    fn roadmap_task_roundtrip() {
        let task = RoadmapTask {
            id: "#16".into(),
            text: "Define magistr-protocol crate".into(),
            checked: false,
            phase_id: "E2".into(),
        };
        let json = serde_json::to_string(&task).unwrap();
        let parsed: RoadmapTask = serde_json::from_str(&json).unwrap();
        assert_eq!(task, parsed);
    }

    #[test]
    fn roadmap_add_task_roundtrip() {
        let params = RoadmapAddTaskParams {
            phase_id: "E2".into(),
            text: "New task".into(),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: RoadmapAddTaskParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, parsed);
    }

    // ── Cost params/results ─────────────────────────────────────────────

    #[test]
    fn cost_extend_budget_roundtrip() {
        let params = CostExtendBudgetParams {
            session_id: "s1".into(),
            new_limit: 10.0,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: CostExtendBudgetParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, parsed);
    }

    // ── System results ──────────────────────────────────────────────────

    #[test]
    fn system_version_result_roundtrip() {
        let result = SystemVersionResult {
            protocol_version: "0.1.0".into(),
            server_version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SystemVersionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    // ── Migration params/results ─────────────────────────────────

    #[test]
    fn fleet_drain_params_default_to_cordon_and_evict() {
        let parsed: FleetDrainParams =
            serde_json::from_str(r#"{"node_id": "n1"}"#).expect("minimal params");
        assert!(parsed.draining, "the bare request drains");
        assert!(parsed.evict, "and hands the work back");
    }

    #[test]
    fn fleet_drain_result_roundtrip() {
        let result = FleetDrainResult {
            node_id: "n1".into(),
            draining: true,
            handed_off: vec!["j1".into(), "j2".into()],
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: FleetDrainResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(result, parsed);
    }

    #[test]
    fn a_work_assignment_carries_the_state_to_resume_from() {
        // The session's own creation time doubles as the capture
        // instant, which keeps a clock crate out of the protocol crate.
        let session = magistr_engine::EngineSession::new("sess-1");
        let now = session.created_at();
        let snapshot = SessionSnapshot::capture(
            &session,
            IterationState {
                iteration: 2,
                max_iterations: 10,
                last_status: magistr_engine::IterationStatus::None,
                spec_checked: 1,
                spec_total: 5,
            },
            now,
        );
        let assignment = WorkAssignment {
            job_id: "j1".into(),
            project: "p".into(),
            prompt: "go".into(),
            attempt: 2,
            claim_ttl_secs: 900,
            resume_from: Some(snapshot),
        };
        let json = serde_json::to_string(&assignment).expect("serialize");
        let parsed: WorkAssignment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(assignment, parsed);
        assert_eq!(parsed.resume_from.expect("carried").iteration.iteration, 2);
    }

    #[test]
    fn a_first_attempt_assignment_omits_the_resume_state_entirely() {
        let assignment = WorkAssignment {
            job_id: "j1".into(),
            project: "p".into(),
            prompt: "go".into(),
            attempt: 1,
            claim_ttl_secs: 900,
            resume_from: None,
        };
        let json = serde_json::to_string(&assignment).expect("serialize");
        assert!(!json.contains("resume_from"), "{json}");
        let parsed: WorkAssignment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(assignment, parsed);
    }

    // ── Multi-project batch params/results ───────────────────────

    #[test]
    fn batch_submit_params_take_the_minimal_project_shape() {
        let parsed: BatchSubmitParams = serde_json::from_str(
            r#"{"batch_id": "b1", "prompt": "go", "projects": [{"id": "api"}]}"#,
        )
        .expect("minimal params");
        assert_eq!(parsed.projects[0], ProjectDep::new("api"));
        assert!(
            parsed.requested_by.is_empty(),
            "an unattributed batch is allowed"
        );
    }

    #[test]
    fn batch_submit_params_round_trip_with_deps_and_a_worktree() {
        let params = BatchSubmitParams {
            batch_id: "b1".into(),
            prompt: "go".into(),
            projects: vec![
                ProjectDep::new("api").in_worktree("/repo"),
                ProjectDep::new("web").after(["api"]).in_worktree("/repo"),
            ],
            requested_by: "client-1".into(),
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let parsed: BatchSubmitParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, params);
    }

    #[test]
    fn a_batch_rollup_round_trips_whole() {
        let result = BatchSubmitResult {
            batch_id: "b1".into(),
            order: vec!["api".into(), "web".into()],
            released: vec!["api".into()],
            status: BatchStatus::from_rows(
                "b1",
                vec![
                    BatchRow {
                        project: "api".into(),
                        state: magistr_engine::BatchState::Queued,
                        job_id: "b1:api".into(),
                    },
                    BatchRow {
                        project: "web".into(),
                        state: magistr_engine::BatchState::Waiting,
                        job_id: "b1:web".into(),
                    },
                ],
            ),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: BatchSubmitResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, result);
        assert_eq!(parsed.status.queued, 1);
        assert_eq!(parsed.status.waiting, 1);
    }

    #[test]
    fn a_batch_status_result_defaults_its_sweep_lists() {
        let json = r#"{"status": {"batch_id": "b1", "total": 0, "waiting": 0, "queued": 0,
            "running": 0, "succeeded": 0, "failed": 0, "skipped": 0, "settled": true,
            "rows": []}}"#;
        let parsed: BatchStatusResult = serde_json::from_str(json).expect("minimal result");
        assert!(parsed.released.is_empty());
        assert!(parsed.reclaimed.is_empty());
        assert!(parsed.status.settled);
    }

    #[test]
    fn system_capabilities_result_roundtrip() {
        let result = SystemCapabilitiesResult {
            domains: vec!["session".into(), "workflow".into()],
            methods: method::ALL.iter().map(|s| (*s).to_owned()).collect(),
            notifications: vec!["session.event".into()],
            transports: vec!["stdio".into(), "tcp".into()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SystemCapabilitiesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }
}
