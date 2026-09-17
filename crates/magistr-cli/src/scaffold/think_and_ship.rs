//! The scaffold's view of think-and-ship: the
//! `<!-- think-and-ship -->` section `magistr init` keeps in `CLAUDE.md`.
//!
//! The section keeps the markers `think-and-ship init --full` uses, so a
//! file written by either tool is recognised and re-spliced by the other; the
//! body is magistr's own — it names the `magistr` server as the one process
//! over the store (tas-ship-state-two-servers) and carries magistr's
//! verification commands. `CLAUDE.md` itself stays user-owned
//! (`NeverOverwrite`); only the marked block is ours.

use std::collections::HashMap;
use std::path::Path;

use crate::error::{MagistrError, Result};

/// Opening marker of the generated section (shared with `think-and-ship init`).
pub(crate) const CLAUDE_MD_MARKER: &str = "<!-- think-and-ship -->";

/// Closing marker of the generated section (shared with `think-and-ship init`).
pub(crate) const CLAUDE_MD_END_MARKER: &str = "<!-- /think-and-ship -->";

/// Write or refresh the think-and-ship section of `<dir>/CLAUDE.md`.
///
/// Splices between the markers when they exist, appends the block when they
/// don't, and creates the file when there is none. Returns whether the file
/// changed. A `CLAUDE.md` whose section already matches is left untouched so
/// re-running `magistr init` is a no-op.
pub(crate) fn upsert_claude_md_section(dir: &Path, vars: &HashMap<String, String>) -> Result<bool> {
    let path = dir.join("CLAUDE.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let section = build_section(vars);
    let next = splice(&existing, &section);
    if next == existing {
        return Ok(false);
    }
    std::fs::write(&path, next).map_err(|e| MagistrError::io(&path, e))?;
    Ok(true)
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
            "\n## Verification\n\nThis is a {name} project. Use these commands to verify changes:\n\n{}\n",
            cmds.join("\n")
        )
    };

    format!(
        "{CLAUDE_MD_MARKER}
# think-and-ship

The four tool families below are served **in-process by the `magistr` MCP server** (`magistr mcp-serve`) over one think-and-ship store. Do not also configure a standalone `think-and-ship serve` on this project: two processes over one store each hold their own copy of the execution and roadmap state (persistence is merge-on-save, never reload-on-read), so a write through one is invisible to the other's guards until restart. `magistr init` retires such an entry.

## Tool families

| Family | Purpose | Key tools |
|--------|---------|-----------|
| **think_*** | Reasoning trace: record steps, branch hypotheses, pin conclusions | `think_record_step`, `think_pin_step`, `think_trace_checkpoint` |
| **ship_*** | Execution trace: objectives, tasks, actions, quality gates | `ship_set_objective`, `ship_plan`, `ship_start`, `ship_record`, `ship_check`, `ship_finalize` |
| **roadmap_*** | The long-horizon plan-of-plans driving both | `roadmap_status`, `roadmap_next`, `roadmap_start_chunk`, `roadmap_complete_chunk`, `roadmap_export` |
| **signal_*** | Stakeholder signals: capture, research, surface, promote to the roadmap | `signal_capture`, `signal_pending`, `signal_research`, `signal_promote` |

## Cross-referencing

Link reasoning to execution:
- On `think_record_step`, pass `execution_ref: \"task:<id>\"` to point at a ship_* task.
- On `ship_record`, pass `think_step: <N>` to point back at the motivating think_* step.

Both halves resolve the same project identity from the working directory so
traces auto-correlate.

## Quick-start workflow

First read `ship_status` and, for roadmap work, `roadmap_status`. Resume a matching unfinished objective and keep its plan, task statuses, actions, checks, and reasoning. Continue an already active task without starting it again; do not restart completed tasks. Preserve conflicting active work and report the conflict instead of resetting it. For `/next`, follow the selected chunk and the recovery contract in `.claude/skills/next/SKILL.md`; pending work is considered only when no authorized in-progress work remains.

1. `ship_set_objective` — define the goal only when no unfinished objective exists
2. `ship_plan` — keep existing tasks and add only missing work
3. `think_record_step` — record the question, current evidence, and planned next check
4. `ship_start` a pending task, or continue the active task; then `ship_record` → `ship_complete` — do the work, recording findings and changed conclusions as evidence arrives
5. `ship_check` — execute and record the relevant checks
6. `think_record_step` — synthesize the findings, link the steps used, and state remaining uncertainty
7. `ship_finalize` — finalize the objective after its acceptance criteria and checks pass

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
            // A terminated block is followed by its own newline; a legacy
            // block bounded at the next heading is not — keep the heading on
            // its own line either way.
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

/// Byte offsets bounding the existing section, marker to marker. A legacy
/// block without the closing marker runs to the next top-level heading after
/// its own (or to the end of the file), exactly as `think-and-ship init`
/// bounds it.
fn span(text: &str) -> Option<(usize, usize)> {
    let start = text.find(CLAUDE_MD_MARKER)?;
    if let Some(end) = text[start..].find(CLAUDE_MD_END_MARKER) {
        return Some((start, start + end + CLAUDE_MD_END_MARKER.len()));
    }
    let body = &text[start..];
    let after_own_heading = body.find("\n# ").map(|i| i + 1).unwrap_or(body.len());
    let next = body[after_own_heading..]
        .find("\n# ")
        .map(|i| after_own_heading + i + 1)
        .unwrap_or(body.len());
    Some((start, start + next))
}

fn capitalise(s: &str) -> String {
    let mut chars = s.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "think_and_ship.tests.rs"]
mod tests;
