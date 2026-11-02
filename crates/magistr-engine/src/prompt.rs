//! Prompt builder for workflow agent sessions.
//!
//! Constructs specialized prompts for `/next` and `/next-loop` commands
//! that embed workflow instructions and project context. IO-free — takes
//! pre-built config and spec summary, returns prompt strings.

use crate::spec::SpecSummary;

/// Minimal project configuration needed for prompt construction.
///
/// Infrastructure adapters construct this from the full `ProjectConfig`
/// in `magistr-cli`. Only the fields actually used in prompt text are included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptConfig {
    /// Spec file path (e.g., "PLAN.md"). `None` on a magistr
    /// project, where the plan lives in the native engines and the agent
    /// reads it through `plan_next` instead of a file.
    pub spec_file: Option<String>,
    /// Check command (e.g., "cargo check --all-targets").
    pub check_command: String,
    /// Test command (e.g., "cargo nextest run").
    pub test_command: String,
    /// Lint command (e.g., "cargo clippy -- -D warnings").
    pub lint_command: String,
    /// Format command (e.g., "cargo fmt").
    pub format_command: String,
}

impl PromptConfig {
    /// Shared recording contract for live agents and generated project instructions.
    pub const REASONING_GUIDANCE: &'static str = include_str!("prompt_reasoning.md");

    /// Shared recovery contract for native iterations and generated next skills.
    pub const NEXT_RECOVERY_GUIDANCE: &'static str = include_str!("prompt_recovery.md");

    /// Build the prompt for a `/next` iteration.
    ///
    /// Embeds /next-specific instructions (stop rule, plan entry labeling, plan
    /// tools) alongside project-specific context. The base R-P-E-V workflow is
    /// loaded via `.claude/rules/workflow.md` in the system prompt — not duplicated here.
    pub fn build_next_prompt(&self, spec_summary: &SpecSummary) -> String {
        let mut prompt = String::with_capacity(4096);

        prompt.push_str("# Task: Complete One Plan Item\n\n");

        // Progress context
        prompt.push_str(&format!(
            "## Current Progress\n\n\
             - Plan: {}\n\
             - Progress: {}/{} items complete ({:.0}%)\n\
             - Remaining: {} items\n\n",
            self.plan_source(),
            spec_summary.checked,
            spec_summary.total,
            spec_summary.progress() * 100.0,
            spec_summary.unchecked,
        ));

        // Native selection contains one chosen task; file summaries preview remaining items.
        if !spec_summary.unchecked_items.is_empty() {
            prompt.push_str(if self.spec_file.is_none() {
                "### Selected Work\n\n"
            } else {
                "### Next Unchecked Items\n\n"
            });
            for (i, item) in spec_summary.unchecked_items.iter().take(10).enumerate() {
                prompt.push_str(&format!("{}. {}\n", i + 1, item));
            }
            if spec_summary.unchecked_items.len() > 10 {
                prompt.push_str(&format!(
                    "\n... and {} more items\n",
                    spec_summary.unchecked_items.len() - 10
                ));
            }
            prompt.push('\n');
        }

        // Recover saved work before any new lifecycle operation.
        prompt.push_str(Self::NEXT_RECOVERY_GUIDANCE);
        prompt.push('\n');
        prompt.push_str(NEXT_WORKFLOW_INSTRUCTIONS);
        prompt.push('\n');
        prompt.push_str(Self::REASONING_GUIDANCE);

        // Project-specific config
        prompt.push_str(&format!(
            "\n## Project Configuration\n\n\
             - **Plan:** {}\n\
             - **Check command:** `{}`\n\
             - **Test command:** `{}`\n\
             - **Lint command:** `{}`\n\
             - **Format command:** `{}`\n",
            self.plan_source(),
            self.check_command,
            self.test_command,
            self.lint_command,
            self.format_command,
        ));

        prompt
    }

    /// Where the agent finds the plan: the configured spec file, or the
    /// magistr engines when the project declares no spec file.
    fn plan_source(&self) -> String {
        self.spec_file.as_ref().map_or_else(
            || "magistr (`plan_next` / `plan_status`)".to_string(),
            |file| format!("`{file}`"),
        )
    }
}

/// `/next`-specific instructions appended to the user prompt.
///
/// The base Research-Plan-Execute-Verify workflow is already loaded into the
/// system prompt via `.claude/rules/workflow.md` (see `build_system_prompt()`).
/// This constant contains ONLY additions specific to `/next` iterations:
/// the magistr task loop, stop rule, plan entry labeling, and status
/// tracking.
pub(crate) const NEXT_WORKFLOW_INSTRUCTIONS: &str = r#"## Workflow

Follow the Research-Plan-Execute-Verify workflow from the project rules.
Call `magistr_set_phase` at each phase transition to record the current workflow phase.

The plan, plan and gates live in native Magistr state. Start with `context`; use `discover` for advanced operation schemas and `execute` with `magistr.call(name, args)` to call them. Common work uses the direct tools below:

Choose `verification: "focused"` for bounded work and run the relevant checks. A full suite requires an explicit request or a stated cross-cutting risk.

- `plan_status` + `run_status` → recover the selected task and matching unfinished goal first
- `plan_next` → select pending work only when there is no resumable in-progress task; `plan_start_task(id)` starts a pending task once
- `run_open` + `arrange_steps` → create a missing goal and plan; keep the matching unfinished goal and saved plan when resuming
- `think_record_step` → record the plan, material findings and investigations as work proceeds, then the verified outcome
- `check_step` / `magistr_gate` → every quality gate, recorded with its real exit code
- `plan_complete_task(id, step_ref: "step:<id>")` + `run_finish` → done

### Execute Phase — restrictions

During Execute, implement and test the change, recording material findings and investigations as they occur. Do NOT:
- Call `plan_complete_task` or `plan_add_task` — plan updates belong in Verify
- Commit code — commits belong in Verify
- Run full quality gates — save that for Verify (periodic `cargo check` is fine)

### Verify Phase — /next-specific steps

After quality gates pass (each recorded through `check_step`), complete these additional steps:

1. **Update the plan** (MANDATORY)
   - `plan_complete_task(id: "<task>", step_ref: "step:<verify step>")` for the task you implemented
   - `plan_add_task(status: "idea", ...)` for work you discovered — never promote it to to_do yourself
   - `plan_update_task` when the task's description or acceptance no longer matches reality
   - Preserve the project's plan export policy. When `plan_exports` is false, never generate PLAN.md or PLAN.json or invoke `plan_export`.
2. **Finalize** — record the final synthesis with `think_record_step`, linking the findings used and any unresolved questions; then close the goal with `run_finish(summary: ...)`
3. **Commit** with a conventional commit message (`feat:`, `fix:`, `refactor:`, `test:`)
   - Include only files within the authorized change; preserve other sessions' work
4. **Append summary** to `.magistr/iteration-log.md`
5. **Mark done** — call `magistr_set_phase(phase: "done")`
6. **Report** what was done, quality gate status, and what's next (`plan_next`)

**IMPORTANT — MANDATORY STOP RULE:**
- After completing the verify phase, you MUST STOP. End your turn. Do not continue.
- Do NOT invoke `/next` or `/next-loop` yourself. The orchestrator handles iteration chaining.
- Do NOT start planning or implementing the next task. Each iteration runs in a fresh context window.
- If you keep working past verify, you will accumulate stale context, miss the iteration boundary, and the recorded phase will not track your progress correctly.
- Your job is ONE iteration: Research → Plan → Execute → Verify → STOP.

**Plan Entry Labeling (REQUIRED):**
Prefix each plan entry with its workflow phase tag:
- `[Research]` — web search, documentation scraping
- `[Plan]` — reading, exploring, writing the run plan
- `[Execute]` — writing code, tests, TDD
- `[Verify]` — quality gate, plan update, commit

**Plan Entry Status Updates (REQUIRED):**
Keep plan entry statuses up to date — the run's step plan retains each step's status.

- Mark entries as `in_progress` when you start working on them
- Mark entries as `completed` when you finish them
- Only one entry should be `in_progress` at a time
- Update the plan with every phase transition — all entries from the previous phase should be `completed` before starting the next phase
- On resume, continue an already active step without calling `start_step` again; do not restart completed entries or discard their progress and check results
"#;

#[cfg(test)]
#[path = "prompt.tests.rs"]
mod tests;
