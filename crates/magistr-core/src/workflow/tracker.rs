//! Evidence-based workflow phase tracker.
//!
//! Replaces heuristic phase detection with an accumulative evidence model.
//! Each phase transition requires **multiple corroborating signals** instead
//! of firing on a single tool call. This prevents false transitions from:
//! - TDD test runs being mistaken for quality gates
//! - Config file edits being mistaken for source edits
//! - Sub-phase completions being mistaken for phase completions
//!
//! The tracker also intercepts Claude Code's `TodoWrite` tool calls to get
//! structured progress data (actual task items and their statuses).

use crate::paths::{is_config_path, is_iteration_log, is_plan_file, is_task_file};
use crate::types::QualityGate;

// ============================================================================
// Phase & Evidence Types
// ============================================================================

// Canonical definition lives in magistr-core. Re-export for backward compatibility.
pub use crate::WorkflowPhase;

/// Evidence accumulated during the **Research** phase.
#[derive(Debug, Clone, Default)]
pub struct ResearchEvidence {
    /// Number of web_search / firecrawl / web_fetch tool calls observed.
    pub research_tool_calls: u32,
    /// Number of exploration tool calls (Read, Glob, Grep, magistr-scope, etc.)
    pub exploration_tool_calls: u32,
}

/// Evidence accumulated during the **Plan** phase.
#[derive(Debug, Clone, Default)]
pub struct PlanEvidence {
    /// The plan was recorded in the execution trace: a `ship_plan` (or
    /// `ship_set_objective`) call was observed. This is the plan signal the
    /// think-and-ship workflow produces; the plan file below is legacy.
    pub plan_recorded: bool,
    /// `.magistr/current-plan.md` was written (legacy plan-file workflow).
    pub plan_file_written: bool,
    /// `.magistr/current-task.md` was written.
    pub task_file_written: bool,
    /// Number of source file edits (should be 0 during planning).
    pub source_edits: u32,
}

impl PlanEvidence {
    /// A plan exists in either form — recorded through `ship_plan` or written
    /// to the legacy plan file. The tracker's transitions key on this, so the
    /// plan file is accepted but never demanded.
    #[must_use]
    pub fn has_plan(&self) -> bool {
        self.plan_recorded || self.plan_file_written
    }
}

/// Evidence accumulated during the **Execute** phase.
#[derive(Debug, Clone, Default)]
pub struct ExecuteEvidence {
    /// Number of source files created or edited.
    pub source_files_touched: u32,
    /// Number of test files created or edited.
    pub test_files_touched: u32,
    /// Number of test commands run (TDD cycles).
    pub test_runs: u32,
    /// Whether a full quality gate command was run (test + lint).
    pub quality_gate_ran: bool,
}

/// Evidence accumulated during the **Verify** phase.
#[derive(Debug, Clone, Default)]
pub struct VerifyEvidence {
    /// Quality gate command was executed.
    pub quality_gate_ran: bool,
    /// Quality gate passed (no error in tool result).
    pub quality_gate_passed: bool,
    /// Spec file (ROADMAP) was updated.
    pub spec_updated: bool,
    /// A git commit was made.
    pub committed: bool,
    /// Iteration log was updated.
    pub log_updated: bool,
    /// State files were cleaned up.
    pub state_cleaned: bool,
}

/// A single todo item from the agent's `TodoWrite` tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

/// Status of a tracked todo item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

// ============================================================================
// Tool Event (the single input type for the tracker)
// ============================================================================

/// A normalized tool event extracted from the message stream.
///
/// The handler converts raw `ContentBlock::ToolUse` into this type,
/// keeping the tracker decoupled from SDK message types.
#[derive(Debug, Clone)]
pub enum ToolEvent {
    /// A file was written or edited.
    FileWrite { path: String },
    /// A bash command was executed.
    BashCommand { command: String },
    /// A bash command completed with a result.
    BashResult { command: String, is_error: bool },
    /// The agent wrote its todo list.
    TodoWrite { todos: Vec<TodoItem> },
    /// A git commit was made (detected from bash).
    GitCommit,
    /// An MCP tool was called (web_search, firecrawl, web_fetch, etc.).
    McpToolCall { tool_name: String },
    /// The plan was recorded in the execution trace (`ship_set_objective`,
    /// `ship_plan`) — the think-and-ship replacement for writing a plan file.
    ShipPlan { tool_name: String },
    /// A read/exploration tool was called (Read, Glob, Grep, magistr-scope, etc.).
    Exploration { tool_name: String },
    /// ACP plan entries were updated — carries the classified phase from entries.
    ///
    /// This event allows plan entries to accelerate phase transitions without
    /// silencing the evidence-based tracker. The tracker uses this as an
    /// additional signal alongside tool events.
    PlanUpdate {
        /// Phase classified from the active/latest plan entry content.
        /// `None` if entries are empty or unclassifiable.
        classified_phase: Option<WorkflowPhase>,
        /// Whether all plan entries are completed.
        all_complete: bool,
    },
}

// ============================================================================
// WorkflowTracker
// ============================================================================

/// Evidence-based workflow phase tracker.
///
/// Feed it `ToolEvent`s and it accumulates evidence, transitioning phases
/// only when sufficient corroborating signals exist.
#[derive(Debug, Clone)]
pub struct WorkflowTracker {
    phase: WorkflowPhase,
    research: ResearchEvidence,
    plan: PlanEvidence,
    execute: ExecuteEvidence,
    verify: VerifyEvidence,
    /// Agent's todo items (from TodoWrite).
    todos: Vec<TodoItem>,
    /// Quality gate config (for matching commands).
    quality_gate: Option<QualityGate>,
    /// Last bash command seen (for correlating with BashResult).
    last_bash_command: Option<String>,
    /// Whether research is required before transitioning to Plan.
    research_required: bool,
}

impl WorkflowTracker {
    /// Create a new tracker starting in the Researching phase.
    pub fn new(quality_gate: Option<QualityGate>) -> Self {
        Self {
            phase: WorkflowPhase::Researching,
            research: ResearchEvidence::default(),
            plan: PlanEvidence::default(),
            execute: ExecuteEvidence::default(),
            verify: VerifyEvidence::default(),
            todos: Vec::new(),
            quality_gate,
            last_bash_command: None,
            research_required: true,
        }
    }

    /// Set whether the research guard is active.
    pub fn set_research_required(&mut self, required: bool) {
        self.research_required = required;
    }

    /// Evidence for the current research phase.
    pub fn research_evidence(&self) -> &ResearchEvidence {
        &self.research
    }

    /// Current workflow phase.
    pub fn phase(&self) -> WorkflowPhase {
        self.phase
    }

    /// Force the tracker to a specific phase (used by explicit `magistr_set_phase` calls).
    ///
    /// Force the workflow phase unconditionally. This is an explicit directive
    /// from the orchestration layer (`magistr_set_phase` MCP tool), so it
    /// bypasses all guards (including the research guard). Evidence-based
    /// guards only apply to auto-detected transitions via `observe()`.
    ///
    /// Always returns `None` (success).
    pub fn force_phase(&mut self, phase: WorkflowPhase) -> Option<String> {
        self.phase = phase;
        None
    }

    /// Evidence for the current planning phase.
    pub fn plan_evidence(&self) -> &PlanEvidence {
        &self.plan
    }

    /// Evidence for the current execute phase.
    pub fn execute_evidence(&self) -> &ExecuteEvidence {
        &self.execute
    }

    /// Evidence for the current verify phase.
    pub fn verify_evidence(&self) -> &VerifyEvidence {
        &self.verify
    }

    /// The agent's current todo list (from TodoWrite events).
    pub fn todos(&self) -> &[TodoItem] {
        &self.todos
    }

    /// Process a tool event. Returns a steer message if the agent needs
    /// redirection (e.g., too many exploration calls without web research),
    /// or `None` if everything is fine. Phase changes are applied internally.
    pub fn observe(&mut self, event: &ToolEvent) -> Option<String> {
        let before = self.phase;

        match event {
            ToolEvent::TodoWrite { todos } => {
                self.todos = todos.clone();
            }
            ToolEvent::PlanUpdate {
                classified_phase,
                all_complete,
            } => {
                // Plan entries are an authoritative forward-only signal.
                // They can advance the phase but never regress it.
                if *all_complete {
                    self.phase = WorkflowPhase::Complete;
                } else if let Some(plan_phase) = classified_phase
                    && plan_phase.step_index() > self.phase.step_index()
                {
                    self.phase = *plan_phase;
                }
            }
            _ => {
                self.accumulate(event);
                self.evaluate_transition();
            }
        }

        // Research nudge: after 3+ exploration calls during Research with no
        // web research, warn the agent. Some exploration is fine (understanding
        // what to research), but the agent must do web research before Plan.
        if self.research_required
            && self.phase == WorkflowPhase::Researching
            && self.research.research_tool_calls == 0
            && self.research.exploration_tool_calls >= 3
            && self.research.exploration_tool_calls.is_multiple_of(3)
            && matches!(event, ToolEvent::Exploration { .. })
        {
            return Some(format!(
                "You've made {} exploration calls without any web research. \
                 The Research phase requires at least one web research tool call \
                 (brave_web_search, context7, firecrawl, etc.) before you can \
                 transition to Plan. Do your web research now — best practices, \
                 API docs, known pitfalls for what you're about to build.",
                self.research.exploration_tool_calls,
            ));
        }

        if self.phase != before {
            // Phase changed — no steer needed, caller should update UI
            return None;
        }

        None
    }

    /// Accumulate evidence from a tool event (phase-specific).
    fn accumulate(&mut self, event: &ToolEvent) {
        match self.phase {
            WorkflowPhase::Researching => self.accumulate_research(event),
            WorkflowPhase::Planning => self.accumulate_plan(event),
            WorkflowPhase::Executing => self.accumulate_execute(event),
            WorkflowPhase::Verifying => self.accumulate_verify(event),
            WorkflowPhase::Complete => {}
        }
    }

    fn accumulate_research(&mut self, event: &ToolEvent) {
        match event {
            ToolEvent::McpToolCall { .. } => {
                self.research.research_tool_calls += 1;
            }
            ToolEvent::Exploration { .. } => {
                self.research.exploration_tool_calls += 1;
            }
            ToolEvent::BashCommand { .. } => {
                // Bash during research counts as exploration
                self.research.exploration_tool_calls += 1;
            }
            ToolEvent::ShipPlan { .. } => {
                self.plan.plan_recorded = true;
            }
            // File writes during research are forwarded to plan evidence so
            // the Planning→Executing transition has data if research and
            // planning overlap.
            ToolEvent::FileWrite { path } => {
                if is_plan_file(path) {
                    self.plan.plan_file_written = true;
                } else if is_task_file(path) {
                    self.plan.task_file_written = true;
                } else if is_source_file(path) {
                    // Source edits during Researching — agent skipped planning
                    self.plan.source_edits += 1;
                }
            }
            _ => {}
        }
    }

    fn accumulate_plan(&mut self, event: &ToolEvent) {
        match event {
            ToolEvent::ShipPlan { .. } => {
                self.plan.plan_recorded = true;
            }
            ToolEvent::FileWrite { path } => {
                if is_plan_file(path) {
                    self.plan.plan_file_written = true;
                } else if is_task_file(path) {
                    self.plan.task_file_written = true;
                } else if !is_config_path(path) {
                    self.plan.source_edits += 1;
                }
            }
            _ => {}
        }
    }

    fn accumulate_execute(&mut self, event: &ToolEvent) {
        match event {
            ToolEvent::FileWrite { path } => {
                if is_config_path(path) {
                    return;
                }
                if is_test_file(path) {
                    self.execute.test_files_touched += 1;
                } else if is_source_file(path) {
                    self.execute.source_files_touched += 1;
                }
            }
            ToolEvent::BashCommand { command } => {
                self.last_bash_command = Some(command.clone());
                if is_test_only_command(command) {
                    self.execute.test_runs += 1;
                }
                if is_quality_gate_command(command, &self.quality_gate) {
                    self.execute.quality_gate_ran = true;
                }
            }
            _ => {}
        }
    }

    fn accumulate_verify(&mut self, event: &ToolEvent) {
        match event {
            ToolEvent::FileWrite { path } => {
                if is_spec_file(path) {
                    self.verify.spec_updated = true;
                } else if is_iteration_log(path) {
                    self.verify.log_updated = true;
                }
            }
            ToolEvent::BashCommand { command } => {
                self.last_bash_command = Some(command.clone());
                if is_quality_gate_command(command, &self.quality_gate) {
                    self.verify.quality_gate_ran = true;
                }
                if command.contains("git commit") {
                    self.verify.committed = true;
                }
            }
            ToolEvent::BashResult { is_error, .. } => {
                // If the last command was a quality gate, record pass/fail.
                if let Some(ref cmd) = self.last_bash_command
                    && is_quality_gate_command(cmd, &self.quality_gate)
                {
                    self.verify.quality_gate_ran = true;
                    self.verify.quality_gate_passed = !is_error;
                }
            }
            ToolEvent::GitCommit => {
                self.verify.committed = true;
            }
            _ => {}
        }
    }

    /// Evaluate whether evidence is sufficient for a phase transition.
    fn evaluate_transition(&mut self) {
        match self.phase {
            WorkflowPhase::Researching => {
                // Research guard: require at least one research tool call before
                // allowing any forward transition.
                if self.research_required && self.research.research_tool_calls == 0 {
                    return;
                }

                // Direct skip: source edits during Researching → Executing
                // (agent skipped explicit planning phase but did research)
                if self.plan.source_edits > 0 && !self.plan.has_plan() {
                    self.phase = WorkflowPhase::Executing;
                    return;
                }
                // Plan recorded (ship_plan) or plan file written → Planning
                if self.plan.has_plan() || self.plan.task_file_written {
                    self.phase = WorkflowPhase::Planning;
                }
            }
            WorkflowPhase::Planning => {
                // Transition when a plan exists (ship_plan recorded, or the
                // legacy plan file) AND a source edit happens. The source edit
                // is the definitive signal that execution started.
                if self.plan.has_plan() && self.plan.source_edits > 0 {
                    self.phase = WorkflowPhase::Executing;
                }
            }
            WorkflowPhase::Executing => {
                // Transition when:
                // - At least one source file was touched (actual work done), AND
                // - A quality gate command was run (signals intent to verify)
                if self.execute.source_files_touched > 0 && self.execute.quality_gate_ran {
                    self.phase = WorkflowPhase::Verifying;
                }
            }
            WorkflowPhase::Verifying => {
                // Transition when we have strong evidence of completion:
                // - Spec updated (ROADMAP checkboxes), OR
                // - Git commit made
                // PLUS at least one of: quality gate ran
                let has_closure = self.verify.spec_updated || self.verify.committed;
                let has_gate = self.verify.quality_gate_ran;
                if has_closure && has_gate {
                    self.phase = WorkflowPhase::Complete;
                }
            }
            WorkflowPhase::Complete => {}
        }
    }

    /// Reset for a new iteration (keeps quality_gate config).
    pub fn reset(&mut self) {
        self.phase = WorkflowPhase::Researching;
        self.research = ResearchEvidence::default();
        self.plan = PlanEvidence::default();
        self.execute = ExecuteEvidence::default();
        self.verify = VerifyEvidence::default();
        self.todos.clear();
        self.last_bash_command = None;
    }
}

// ============================================================================
// Path Classification Helpers
// ============================================================================

/// Extract a file path from tool input, checking multiple field names.
///
/// ACP backends use `path`, Claude Code uses `file_path`. If neither is present
/// in the input JSON, falls back to ACP `locations` array, then tries to
/// extract a path from the ACP tool title (e.g., "Editing src/main.rs").
fn extract_file_path(
    input: &serde_json::Value,
    locations: &[crate::ToolLocation],
    tool_title: &str,
) -> Option<String> {
    // Check common field names in the tool input JSON
    let from_input = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .or_else(|| input.get("file"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    if let Some(p) = from_input {
        return Some(p.to_string());
    }

    // Fallback: ACP locations array (always present for file-modifying tools)
    if let Some(loc) = locations.first() {
        return Some(loc.path.clone());
    }

    // Last resort: parse path from ACP tool title.
    // ACP titles for file operations look like "Editing src/main.rs",
    // "Creating .magistr/current-plan.md", "Reading config.toml", etc.
    extract_path_from_title(tool_title)
}

/// Try to extract a file path from an ACP tool title string.
///
/// Handles patterns like:
/// - "Editing src/main.rs"
/// - "Creating /Users/foo/project/.magistr/current-plan.md"
/// - "Reading config.toml"
/// - "Writing tests/foo_test.rs"
fn extract_path_from_title(title: &str) -> Option<String> {
    let trimmed = title.trim();
    let rest = trimmed
        .strip_prefix("Editing ")
        .or_else(|| trimmed.strip_prefix("Creating "))
        .or_else(|| trimmed.strip_prefix("Reading "))
        .or_else(|| trimmed.strip_prefix("Writing "))
        .or_else(|| trimmed.strip_prefix("Viewing "))
        .or_else(|| trimmed.strip_prefix("Deleting "))
        .or_else(|| trimmed.strip_prefix("Moving "))
        .or_else(|| trimmed.strip_prefix("Modifying "))?;

    let path = rest.trim();
    if path.is_empty() {
        return None;
    }
    // Basic sanity: must contain a dot (file extension) or slash (path separator)
    if path.contains('.') || path.contains('/') {
        Some(path.to_string())
    } else {
        None
    }
}

/// Classify a tool event purely from the ACP tool title string.
///
/// ACP tool calls have human-readable titles like:
/// - "Editing src/main.rs" → FileWrite
/// - "Creating .magistr/current-plan.md" → FileWrite
/// - "Viewing /path/to/file.rs" → Exploration
/// - "Reading config.toml" → Exploration
/// - "Searching for 'query...'" → McpToolCall (research)
/// - "Running cargo test" → BashCommand
/// - "Check baseline build status cd /path && cargo check..." → BashCommand
fn tool_event_from_title(title: &str) -> Option<ToolEvent> {
    let lower = title.to_lowercase();

    // File writes: "Editing", "Creating", "Writing", "Modifying", "Deleting", "Moving"
    if (lower.starts_with("editing ")
        || lower.starts_with("creating ")
        || lower.starts_with("writing ")
        || lower.starts_with("modifying "))
        && let Some(path) = extract_path_from_title(title)
    {
        return Some(ToolEvent::FileWrite { path });
    }

    if (lower.starts_with("deleting ") || lower.starts_with("moving "))
        && let Some(path) = extract_path_from_title(title)
    {
        return Some(ToolEvent::FileWrite { path });
    }

    // Exploration: "Viewing", "Reading", "Listing"
    if lower.starts_with("viewing ")
        || lower.starts_with("reading ")
        || lower.starts_with("listing ")
    {
        return Some(ToolEvent::Exploration {
            tool_name: title.to_string(),
        });
    }

    // Research: "Searching for..."
    if lower.starts_with("searching ") {
        return Some(ToolEvent::McpToolCall {
            tool_name: title.to_string(),
        });
    }

    // Execution: "Running ...", "Check ..." (bash commands in title)
    if lower.starts_with("running ") {
        let rest = title[8..].trim();
        if !rest.is_empty() {
            if rest.contains("git commit") {
                return Some(ToolEvent::GitCommit);
            }
            return Some(ToolEvent::BashCommand {
                command: rest.to_string(),
            });
        }
    }

    // Bash commands embedded in titles: "Check baseline build status cd /path && cargo ..."
    if lower.contains("cargo ")
        || lower.contains("npm ")
        || lower.contains("make ")
        || lower.contains("git commit")
    {
        if lower.contains("git commit") {
            return Some(ToolEvent::GitCommit);
        }
        return Some(ToolEvent::BashCommand {
            command: title.to_string(),
        });
    }

    None
}

/// The think-and-ship verbs that record a plan: `ship_set_objective` and
/// `ship_plan`, under any MCP prefix (`mcp__magistr__ship_plan`,
/// `magistr-ship_plan`, bare).
fn is_ship_plan_tool(name: &str) -> bool {
    name.ends_with("ship_plan") || name.ends_with("ship_set_objective")
}

fn is_spec_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("roadmap") || lower.contains("spec.md") || lower.contains("todo.md")
}

fn is_test_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("test") || lower.contains("spec.") || lower.ends_with("_test.go")
}

fn is_source_file(path: &str) -> bool {
    // Anything that isn't config, spec, or pure docs
    !is_config_path(path) && !is_spec_file(path) && !is_iteration_log(path)
}

/// Returns true if the command runs ONLY tests (TDD cycle), not the full gate.
///
/// Delegates to `crate::tools::gate_patterns` — the single source of truth.
pub fn is_test_only_command(cmd: &str) -> bool {
    crate::tools::gate_patterns::GatePatterns::is_test_only(cmd)
}

/// Returns true if a bash command is a full quality gate (requires lint).
///
/// Delegates to `crate::tools::gate_patterns` — the single source of truth.
pub fn is_quality_gate_command(cmd: &str, gate: &Option<QualityGate>) -> bool {
    crate::tools::gate_patterns::GatePatterns::is_quality_gate(cmd, gate)
}

// ============================================================================
// ACP Plan-Driven Phase Classification
// ============================================================================

/// Derive the current workflow phase from ACP plan entries.
///
/// This is the primary phase tracking mechanism — it reads the agent's own
/// plan entries (structured data from ACP) instead of guessing from tool
/// call side-effects.
///
/// Classification logic:
/// 1. Empty entries → `None` (no plan yet)
/// 2. All entries Completed → `Complete`
/// 3. First InProgress entry → classify its content to a phase
/// 4. No InProgress but not all done → phase of last completed entry
/// 5. All Pending → `Planning` (plan just created, nothing started)
pub fn classify_plan_phase(entries: &[crate::AcpPlanEntry]) -> Option<WorkflowPhase> {
    use crate::PlanEntryStatus;

    if entries.is_empty() {
        return None;
    }

    // All completed → Complete
    if entries
        .iter()
        .all(|e| e.status == PlanEntryStatus::Completed)
    {
        return Some(WorkflowPhase::Complete);
    }

    // Phase = phase of the first InProgress entry
    if let Some(active) = entries
        .iter()
        .find(|e| e.status == PlanEntryStatus::InProgress)
    {
        return Some(classify_entry_phase(&active.content));
    }

    // No InProgress entries but not all completed — transitioning between phases.
    // Use the last completed entry to infer where we are.
    if let Some(last_done) = entries
        .iter()
        .rev()
        .find(|e| e.status == PlanEntryStatus::Completed)
    {
        return Some(classify_entry_phase(&last_done.content));
    }

    // All Pending → Researching (plan exists but agent hasn't started)
    Some(WorkflowPhase::Researching)
}

/// Classify a single plan entry's content into a workflow phase.
///
/// Primary signal: explicit `[Plan]`, `[Execute]`, `[Verify]` prefix tags.
/// Fallback: keyword matching on content.
pub fn classify_entry_phase(content: &str) -> WorkflowPhase {
    let lower = content.to_lowercase();

    // Explicit phase tags (prompt instructs agent to use these)
    if lower.starts_with("[research]") {
        return WorkflowPhase::Researching;
    }
    if lower.starts_with("[plan]") || lower.starts_with("[planning]") {
        return WorkflowPhase::Planning;
    }
    if lower.starts_with("[execute]")
        || lower.starts_with("[executing]")
        || lower.starts_with("[implement]")
    {
        return WorkflowPhase::Executing;
    }
    if lower.starts_with("[verify]")
        || lower.starts_with("[verifying]")
        || lower.starts_with("[close]")
    {
        return WorkflowPhase::Verifying;
    }

    // Keyword fallback — check research first, then verify, then execute
    if has_research_keywords(&lower) {
        return WorkflowPhase::Researching;
    }
    if has_verify_keywords(&lower) {
        return WorkflowPhase::Verifying;
    }
    if has_execute_keywords(&lower) {
        return WorkflowPhase::Executing;
    }

    // Default to Planning (reading, exploring, etc.)
    WorkflowPhase::Planning
}

/// Returns true if content matches research-phase keywords.
fn has_research_keywords(lower: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "web search",
        "web_search",
        "firecrawl",
        "scrape doc",
        "scrape page",
        "research best practice",
        "research api",
        "search for",
        "look up",
        "web fetch",
    ];
    KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Returns true if content matches verify-phase keywords.
fn has_verify_keywords(lower: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "quality gate",
        "run quality",
        "verify",
        "validate",
        "update roadmap",
        "update spec",
        "check off",
        "mark complete",
        "git commit",
        "commit change",
        "clean up",
        "cleanup",
        "finalize",
        "close iteration",
        "iteration log",
        "iteration summary",
    ];
    KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Returns true if content matches execute-phase keywords.
fn has_execute_keywords(lower: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "implement",
        "write test",
        "create test",
        "add test",
        "write code",
        "build module",
        "create module",
        "tdd",
        "red-green",
        "refactor",
        "add function",
        "add method",
        "add struct",
        "add type",
        "add component",
        "write function",
        "create file",
        "edit source",
        "modify source",
        "code change",
        // Broader patterns for generic plan entries
        "start implement",
        "set up",
        "setup",
        "scaffold",
        "create src",
        "write src",
        "add module",
        "add feature",
        "write module",
        "coding",
        "source file",
        "create struct",
        "create enum",
        "create type",
        "create trait",
        "create class",
        "create component",
        "create service",
        "create handler",
        "write handler",
        "add handler",
        "add endpoint",
        "add route",
        "create route",
    ];
    KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Parse a `TodoWrite` tool_use input into `TodoItem`s.
///
/// Expected JSON shape:
/// ```json
/// { "todos": [{ "id": "1", "content": "...", "status": "pending" }] }
/// ```
pub fn parse_todo_write(input: &serde_json::Value) -> Vec<TodoItem> {
    let Some(todos) = input.get("todos").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    todos
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let content = item.get("content")?.as_str()?.to_string();
            let status = match item.get("status").and_then(|v| v.as_str()) {
                Some("completed") => TodoStatus::Completed,
                Some("in_progress") => TodoStatus::InProgress,
                _ => TodoStatus::Pending,
            };
            Some(TodoItem {
                id,
                content,
                status,
            })
        })
        .collect()
}

/// Extract a `ToolEvent` from a tool kind, name, and input JSON.
///
/// Uses `ToolCategory::classify()` for category dispatch, then extracts event-specific
/// data (file paths, command strings, todo items) from the input JSON.
///
/// Returns `None` for tools that don't produce workflow evidence.
pub fn tool_event_from(
    kind: Option<crate::ToolKind>,
    name: &str,
    input: &serde_json::Value,
    locations: &[crate::ToolLocation],
) -> Option<ToolEvent> {
    use crate::tools::classify::ToolCategory;

    // TodoWrite detection by input fields — ToolCategory::classify can't inspect input,
    // so we check here before delegating to the category-based dispatch.
    if kind == Some(crate::ToolKind::Other)
        && (input.get("action").is_some() || input.get("todos").is_some())
    {
        let todos = parse_todo_write(input);
        return Some(ToolEvent::TodoWrite { todos });
    }

    if is_ship_plan_tool(name) {
        return Some(ToolEvent::ShipPlan {
            tool_name: name.to_string(),
        });
    }

    let category = ToolCategory::classify(kind, name);

    match category {
        ToolCategory::Research => Some(ToolEvent::McpToolCall {
            tool_name: name.to_string(),
        }),

        ToolCategory::Exploration => Some(ToolEvent::Exploration {
            tool_name: name.to_string(),
        }),

        ToolCategory::FileWrite => {
            if let Some(path) = extract_file_path(input, locations, name) {
                Some(ToolEvent::FileWrite { path })
            } else {
                // Input missing — fall through to title-based fallback
                tool_event_from_title(name)
            }
        }

        ToolCategory::Command => {
            let cmd = input
                .get("command")
                .or_else(|| input.get("input"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            if let Some(cmd) = cmd {
                if cmd.contains("git commit") {
                    Some(ToolEvent::GitCommit)
                } else {
                    Some(ToolEvent::BashCommand {
                        command: cmd.to_string(),
                    })
                }
            } else {
                // Input missing — fall through to title-based fallback
                tool_event_from_title(name)
            }
        }

        ToolCategory::Todo => {
            let todos = parse_todo_write(input);
            Some(ToolEvent::TodoWrite { todos })
        }

        // Orchestration, RoadmapMutation, AgentSpawn — no workflow evidence
        ToolCategory::Orchestration | ToolCategory::RoadmapMutation | ToolCategory::AgentSpawn => {
            None
        }

        // Unknown — try title-based fallback for ACP human-readable titles
        // like "Editing src/main.rs", "Running cargo test", etc.
        ToolCategory::Unknown => tool_event_from_title(name),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "tracker.tests.rs"]
mod tests;
