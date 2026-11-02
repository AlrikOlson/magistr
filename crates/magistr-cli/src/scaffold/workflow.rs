//! The marked workflow instructions generated from current public operations.
//! Setup owns this bounded section; the rest of CLAUDE.md stays user-owned.

use std::collections::HashMap;

/// Opening marker of the generated section owned by Magistr.
pub(crate) const CLAUDE_MD_MARKER: &str = "<!-- magistr -->";

/// Closing marker of the generated section (shared with `magistr init`).
pub(crate) const CLAUDE_MD_END_MARKER: &str = "<!-- /magistr -->";

/// `CLAUDE.md` with its magistr section written or refreshed.
///
/// Splices between the markers when they exist and appends the block when
/// they don't. A document whose section already matches comes back
/// unchanged, so the setup plan lists it as unchanged and re-running
/// `magistr init` is a no-op.
pub(crate) fn with_claude_md_section(
    document: &str,
    vars: &HashMap<String, String>,
) -> crate::error::Result<String> {
    if document.contains(CLAUDE_MD_MARKER) && !document.contains(CLAUDE_MD_END_MARKER) {
        return Err(crate::error::MagistrError::Config(
            "CLAUDE.md has an unclosed Magistr section; restore its closing marker before setup"
                .into(),
        ));
    }
    Ok(splice(document, &build_section(vars)))
}

/// The full marked section for a project, including the Verification block
/// when the template vars carry gate commands.
pub(crate) fn build_section(vars: &HashMap<String, String>) -> String {
    let cmds: Vec<String> = ["check_cmd", "test_cmd", "lint_cmd"]
        .iter()
        .filter_map(|k| vars.get(*k))
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| format!("- `{c}`"))
        .collect();
    let verify_block = if cmds.is_empty() {
        String::new()
    } else {
        let name = vars
            .get("language")
            .map(|l| capitalise(l))
            .filter(|l| !l.is_empty() && l != "Unknown")
            .unwrap_or_else(|| "software".to_owned());
        format!(
            "\n## Verification\n\nThis is a {name} project. Select focused checks for the changed behavior. These are the configured full commands; run all of them only for an explicit full-suite request or a concrete cross-cutting risk:\n\n{}\n",
            cmds.join("\n")
        )
    };

    format!(
        "{CLAUDE_MD_MARKER}
# magistr

The `magistr` MCP connection (`magistr mcp --dir .`) provides source tools and the native workflow. Start with `context`. Use direct `search`, `read`, `edit`, `write`, `arrange_steps`, `start_step`, `record_progress`, `check_step`, and `finish_step` for common work. Use read-only `discover` to inspect advanced operation schemas, then `execute` with synchronous `magistr.call(name, args)`. Compositions stop on failure and return receipts; inspect completed effects before recovery. One workflow owner serves the project, and one workspace owner serves each working copy.

## Tool families

| Family | Purpose | Key tools |
|--------|---------|-----------|
| **think_*** | Reasoning trace: record steps, branch hypotheses, pin conclusions | `think_record_step`, `think_pin_step`, `think_trace_checkpoint` |
| **run_*** | Run history: goals, steps, progress, checks and approvals | `run_open`, `arrange_steps`, `start_step`, `record_progress`, `check_step`, `run_finish` |
| **plan_*** | The long-horizon plan-of-plans driving both | `plan_status`, `plan_next`, `plan_start_task`, `plan_complete_task` |
| **signal_*** | Stakeholder signals: capture, research, surface, promote to the plan | `signal_capture`, `signal_pending`, `signal_research`, `signal_promote` |

## Cross-referencing

Link reasoning to runs:
- On `think_record_step`, pass `execution_ref: \"step:<id>\"` to point at a run step.
- On `record_progress`, pass `think_step: <N>` to point back at the motivating think_* step.

Both halves resolve the same project identity from the working directory so
traces auto-correlate.

## Quick-start workflow

First read `run_status` and, for plan work, `plan_status`. Resume a matching unfinished goal and keep its plan, step statuses, progress, checks, and reasoning. Continue an already active step without starting it again; do not restart completed steps. Select the authorized scope with `run_open` when another scope is selected, then read `run_status` again. Scope selection saves unrelated unfinished work and restores a saved goal and plan unchanged; never reset or finalize unrelated work to make room. For `/next`, follow the selected task and the recovery contract in `.claude/skills/next/SKILL.md`; pending work is considered only when no authorized in-progress work remains.

1. `run_open` — select the authorized scope or define a new goal; resume a matching unfinished scope without repeating setup
2. `arrange_steps` — keep existing steps and add only missing work
3. `think_record_step` — record the question, current evidence, and planned next check
4. `start_step` a pending step, or continue the active step; then `record_progress` → `finish_step` — do the work, recording findings and changed conclusions as evidence arrives
5. `check_step` — execute and record the relevant checks
6. `think_record_step` — synthesize the findings, link the steps used, and state remaining uncertainty
7. `run_finish` — finalize the goal after its acceptance criteria and checks pass

{reasoning_guidance}
{verify_block}{CLAUDE_MD_END_MARKER}
",
        reasoning_guidance = magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE,
    )
}

/// Replace the marked block in `text` with `section`, or append it.
fn splice(text: &str, section: &str) -> String {
    match span(text) {
        Some((start, end)) => {
            let mut out = String::with_capacity(text.len() + section.len());
            let rest = &text[end..];
            out.push_str(&text[..start]);
            out.push_str(section.trim_end_matches('\n'));
            // Keep the following user text on its own line.
            if !rest.is_empty() && !rest.starts_with('\n') {
                out.push('\n');
            }
            out.push_str(rest);
            out
        }
        None if text.trim().is_empty() => section.to_owned(),
        None => {
            let mut out = text.trim_end_matches('\n').to_owned();
            out.push_str("\n\n");
            out.push_str(section);
            out
        }
    }
}

/// Byte offsets of the current explicitly closed managed section.
fn span(text: &str) -> Option<(usize, usize)> {
    let start = text.find(CLAUDE_MD_MARKER)?;
    let end = text[start..].find(CLAUDE_MD_END_MARKER)?;
    Some((start, start + end + CLAUDE_MD_END_MARKER.len()))
}

fn capitalise(s: &str) -> String {
    let mut chars = s.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "workflow.tests.rs"]
mod tests;
