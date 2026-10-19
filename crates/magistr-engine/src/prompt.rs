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
    /// Spec file path (e.g., "ROADMAP.md"). `None` on a think-and-ship
    /// project, where the roadmap lives in the native engines and the agent
    /// reads it through `roadmap_next` instead of a file.
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
    /// Embeds /next-specific instructions (stop rule, plan entry labeling, roadmap
    /// tools) alongside project-specific context. The base R-P-E-V workflow is
    /// loaded via `.claude/rules/workflow.md` in the system prompt — not duplicated here.
    pub fn build_next_prompt(&self, spec_summary: &SpecSummary) -> String {
        let mut prompt = String::with_capacity(4096);

        prompt.push_str("# Task: Complete One Roadmap Item\n\n");

        // Progress context
        prompt.push_str(&format!(
            "## Current Progress\n\n\
             - Roadmap: {}\n\
             - Progress: {}/{} items complete ({:.0}%)\n\
             - Remaining: {} items\n\n",
            self.roadmap_source(),
            spec_summary.checked,
            spec_summary.total,
            spec_summary.progress() * 100.0,
            spec_summary.unchecked,
        ));

        // Native selection contains one chosen chunk; file summaries preview remaining items.
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
             - **Roadmap:** {}\n\
             - **Check command:** `{}`\n\
             - **Test command:** `{}`\n\
             - **Lint command:** `{}`\n\
             - **Format command:** `{}`\n",
            self.roadmap_source(),
            self.check_command,
            self.test_command,
            self.lint_command,
            self.format_command,
        ));

        prompt
    }

    /// Where the agent finds the roadmap: the configured spec file, or the
    /// think-and-ship engines when the project declares no spec file.
    fn roadmap_source(&self) -> String {
        self.spec_file.as_ref().map_or_else(
            || "think-and-ship (`roadmap_next` / `roadmap_status`)".to_string(),
            |file| format!("`{file}`"),
        )
    }
}

/// `/next`-specific instructions appended to the user prompt.
///
/// The base Research-Plan-Execute-Verify workflow is already loaded into the
/// system prompt via `.claude/rules/workflow.md` (see `build_system_prompt()`).
/// This constant contains ONLY additions specific to `/next` iterations:
/// the think-and-ship chunk loop, stop rule, plan entry labeling, and status
/// tracking.
pub(crate) const NEXT_WORKFLOW_INSTRUCTIONS: &str = r#"## Workflow

Follow the Research-Plan-Execute-Verify workflow from the project rules.
Call `magistr_set_phase` at each phase transition to record the current workflow phase.

The roadmap, plan and gates live in native Magistr state. Start with `context`; use `discover` for advanced operation schemas and `execute` with `magistr.call(name, args)` to call them. Common work uses the direct tools below:

Choose `verification: "focused"` for bounded work and run the relevant checks. A full suite requires an explicit request or a stated cross-cutting risk.

- `roadmap_status` + `ship_status` → recover the selected chunk and matching unfinished objective first
- `roadmap_next` → select pending work only when there is no resumable in-progress chunk; `roadmap_start_chunk(id)` starts a pending chunk once
- `ship_set_objective` + `plan_work` → create a missing objective and plan; keep the matching unfinished objective and saved plan when resuming
- `think_record_step` → record the plan, material findings and investigations as work proceeds, then the verified outcome
- `check_work` / `magistr_gate` → every quality gate, recorded with its real exit code
- `roadmap_complete_chunk(id, ship_ref: "task:<id>")` + `ship_finalize` → done

### Execute Phase — restrictions

During Execute, implement and test the change, recording material findings and investigations as they occur. Do NOT:
- Call `roadmap_complete_chunk` or `roadmap_add_chunk` — roadmap updates belong in Verify
- Commit code — commits belong in Verify
- Run full quality gates — save that for Verify (periodic `cargo check` is fine)

### Verify Phase — /next-specific steps

After quality gates pass (each recorded through `check_work`), complete these additional steps:

1. **Update the roadmap** (MANDATORY)
   - `roadmap_complete_chunk(id: "<chunk>", ship_ref: "task:<verify task>")` for the chunk you implemented
   - `roadmap_add_chunk(status: "backlog", ...)` for work you discovered — never auto-promote it to pending
   - `roadmap_update_chunk` when the chunk's description or acceptance no longer matches reality
   - Preserve the project's roadmap export policy. When `roadmap_exports` is false, never generate ROADMAP.md or ROADMAP.json or invoke `roadmap_export`.
2. **Finalize** — record the final synthesis with `think_record_step`, linking the findings used and any unresolved questions; then close the objective with `ship_finalize(summary: ...)`
3. **Commit** with a conventional commit message (`feat:`, `fix:`, `refactor:`, `test:`)
   - Include only files within the authorized change; preserve other sessions' work
4. **Append summary** to `.magistr/iteration-log.md`
5. **Mark done** — call `magistr_set_phase(phase: "done")`
6. **Report** what was done, quality gate status, and what's next (`roadmap_next`)

**IMPORTANT — MANDATORY STOP RULE:**
- After completing the verify phase, you MUST STOP. End your turn. Do not continue.
- Do NOT invoke `/next` or `/next-loop` yourself. The orchestrator handles iteration chaining.
- Do NOT start planning or implementing the next chunk. Each iteration runs in a fresh context window.
- If you keep working past verify, you will accumulate stale context, miss the iteration boundary, and the recorded phase will not track your progress correctly.
- Your job is ONE iteration: Research → Plan → Execute → Verify → STOP.

**Plan Entry Labeling (REQUIRED):**
Prefix each plan entry with its workflow phase tag:
- `[Research]` — web search, documentation scraping
- `[Plan]` — reading, exploring, writing the ship plan
- `[Execute]` — writing code, tests, TDD
- `[Verify]` — quality gate, roadmap update, commit

**Plan Entry Status Updates (REQUIRED):**
Keep plan entry statuses up to date — the native execution plan retains each task's status.

- Mark entries as `in_progress` when you start working on them
- Mark entries as `completed` when you finish them
- Only one entry should be `in_progress` at a time
- Update the plan with every phase transition — all entries from the previous phase should be `completed` before starting the next phase
- On resume, continue an already active task without calling `start_work` again; do not restart completed entries or discard their actions and check results
"#;

#[cfg(test)]
#[path = "prompt.tests.rs"]
mod tests;
