//! The pure decision behind `magistr guard`: given the ship/roadmap state, the
//! working tree and the hook's own input, does this hook event block — and why.
//!
//! Kept free of I/O so the rules the `/next` skill promises are tested as
//! plain functions; `cmd_guard` only gathers the inputs and maps the verdict
//! to an exit code.

use std::path::{Path, PathBuf};

use super::guard_routing::workspace_search_refusal;
use crate::cli::GuardEvent;

/// A tree the project has frozen (`.magistr/legacy-gui-ratchet.json`): it may
/// only shrink, so writes under it need a ship objective that says so.
///
/// Nothing here names a particular tree — the declaration file does. That is
/// what let the second freeze (`crates/magistr-gui`) reuse the mechanism the
/// first one (`src/tui`) built without touching this decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrozenTree {
    /// Project-relative root of the frozen tree, e.g. `crates/magistr-gui`.
    pub(crate) path: String,
    /// Words any one of which, in the active objective's scope, unlocks it
    /// (`legacy-gui` for a hotfix, `delete` for a deletion chunk).
    pub(crate) unlock: Vec<String>,
    /// Where work that was refused belongs instead, as the declaration words
    /// it. Appended to the block message; `None` leaves the message silent
    /// about it rather than guessing.
    pub(crate) replacement: Option<String>,
}

/// One recorded quality check, reduced to what the guard looks at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckSummary {
    /// The gate's name (`ship_check(name:)`); re-runs of a gate share it, and
    /// only the latest result per name counts.
    pub(crate) name: String,
    /// A required gate (the default for `ship_check`).
    pub(crate) required: bool,
    /// Its recorded outcome.
    pub(crate) passed: bool,
    /// Recorded from a real exit code (`ship_check(command:)`), not self-reported.
    pub(crate) verified: bool,
}

/// Everything the guard evaluates. Built by `cmd_guard` from the engines,
/// the hook JSON; built by hand in tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GuardInput {
    /// Validated objective status, lowercase (`defined` / `active` /
    /// `completed` / `abandoned`); `None` when no objective exists.
    pub(crate) objective_status: Option<String>,
    /// Number of tasks in the validated execution plan.
    pub(crate) tasks_total: usize,
    /// Every recorded check on the objective.
    pub(crate) checks: Vec<CheckSummary>,
    /// The chunk named by this objective's scope is still `in_progress`.
    pub(crate) chunk_in_progress: bool,
    /// Claude Code already continued once because of a Stop hook.
    pub(crate) stop_hook_active: bool,
    /// `tool_input.command` of a PreToolUse(Bash) hook.
    pub(crate) command: Option<String>,
    /// Validated objective scope; `None` when no objective exists.
    pub(crate) objective_scope: Option<String>,
    /// The tree the project declares frozen; `None` when it declares none.
    pub(crate) frozen: Option<FrozenTree>,
    /// `tool_input.file_path` (or `notebook_path`) of a PreToolUse hook on a
    /// file-editing tool, made relative to the project root; `None` when the
    /// hook carried none or the file lies outside the project.
    pub(crate) write_path: Option<String>,
    /// The project declares a `magistr` server in `.mcp.json`, so
    /// grep, rg, find and sed over the workspace have a replacement.
    pub(crate) workspace: bool,
    /// The canonical project root; relative paths in a Bash command resolve
    /// against it. `None` in a hand-built input disables routing.
    pub(crate) project_dir: Option<PathBuf>,
}

/// What the hook should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GuardVerdict {
    /// Let the event through, optionally saying why the guard did not apply.
    Allow(Option<String>),
    /// Block the event; the reason goes to stderr for the agent to act on.
    Block(String),
}

/// Decide `event` against `input`.
pub(crate) fn evaluate(event: GuardEvent, input: &GuardInput) -> GuardVerdict {
    match event {
        GuardEvent::Stop => evaluate_stop(input),
        GuardEvent::Commit => evaluate_commit(input),
        GuardEvent::Write => evaluate_write(input),
    }
}

/// Write: a file under the frozen tree is only writable while a defined or
/// active ship objective's scope carries one of the tree's unlock words.
/// Everything else — no frozen tree, a path outside it, a hook without a
/// path — passes untouched.
fn evaluate_write(input: &GuardInput) -> GuardVerdict {
    let (Some(frozen), Some(path)) = (&input.frozen, &input.write_path) else {
        return GuardVerdict::Allow(None);
    };
    if !Path::new(path).starts_with(&frozen.path) {
        return GuardVerdict::Allow(None);
    }
    let in_flight = matches!(
        input.objective_status.as_deref(),
        Some("defined" | "active")
    );
    let scope = input
        .objective_scope
        .as_deref()
        .filter(|_| in_flight)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if let Some(word) = frozen
        .unlock
        .iter()
        .find(|w| scope.contains(&w.to_ascii_lowercase()))
    {
        return GuardVerdict::Allow(Some(format!(
            "{} is frozen; this write is admitted because the objective scope says '{word}'",
            frozen.path
        )));
    }
    let hotfix = frozen.unlock.first().map_or_else(
        || "<unlock word>".to_owned(),
        |word| word.to_ascii_lowercase(),
    );
    let elsewhere = frozen
        .replacement
        .as_deref()
        .map(|what| format!(" New work belongs in {what}."))
        .unwrap_or_default();
    GuardVerdict::Block(format!(
        "{path} is under the frozen tree {tree} — a frozen tree only shrinks (see \
         {tree}/README.md). Only a defined/active ship objective whose scope contains \
         one of {unlock:?} may write here: ship_set_objective(scope: \"{hotfix}: \
         <what broke>\") for a hotfix, or a roadmap deletion chunk.{elsewhere}",
        tree = frozen.path,
        unlock = frozen.unlock,
    ))
}

/// Stop checks only the objective's own unfinished roadmap chunk. A dirty
/// checkout cannot identify this run's work: it may contain pre-existing or
/// concurrent changes, so it must never trigger staging or export instructions.
fn evaluate_stop(input: &GuardInput) -> GuardVerdict {
    if input.stop_hook_active {
        return GuardVerdict::Allow(Some(
            "stop hook already active for this turn; not blocking twice".to_owned(),
        ));
    }
    if input.objective_status.as_deref() == Some("completed") && input.chunk_in_progress {
        return GuardVerdict::Block(
            "This work is finished, but its roadmap item is still open. Complete that item \
             with roadmap_complete_chunk using its recorded verification task, or record \
             the work that remains. Preserve unrelated changes."
                .to_owned(),
        );
    }
    GuardVerdict::Allow(None)
}

/// Commit: a `git commit` while a required gate failed, or before any
/// verified gate was recorded for the objective, is refused. Any other Bash
/// command is only routed: grep, rg, find and sed aimed at the workspace
/// are refused in favour of magistr when the project declares it.
/// Projects without an objective pass.
fn evaluate_commit(input: &GuardInput) -> GuardVerdict {
    if !input.command.as_deref().is_some_and(is_git_commit) {
        return evaluate_routing(input);
    }
    if input.objective_status.is_none() {
        return GuardVerdict::Allow(Some(
            "no ship objective is set; this commit is not gated".to_owned(),
        ));
    }
    // A shipped objective has nothing in flight a commit could be skipping —
    // its historical checks (including red runs later re-run green) are
    // records, not state.
    if input.objective_status.as_deref() == Some("completed") {
        return GuardVerdict::Allow(Some(
            "the ship objective is already shipped; this commit is not gated".to_owned(),
        ));
    }
    let latest = latest_check_per_gate(&input.checks);
    if latest.iter().any(|c| c.required && !c.passed) {
        return GuardVerdict::Block(
            "a required quality check is recorded as FAILED — fix it and re-run the \
             affected checks (a named magistr_gate or ship_check(command: …)) before \
             git commit"
                .to_owned(),
        );
    }
    let verified_pass = latest.iter().any(|c| c.required && c.passed && c.verified);
    if !verified_pass && input.tasks_total > 0 {
        return GuardVerdict::Block(
            "no verified quality check is recorded for this objective — run \
             a named magistr_gate or ship_check(command: …) (a real exit \
             code, not a self-reported pass) before git commit"
                .to_owned(),
        );
    }
    GuardVerdict::Allow(None)
}

/// Routing: a workspace search or edit through grep, rg, find or sed is
/// refused with the workspace alternative. Without workspace, or without a
/// project root to resolve paths against, nothing is routed.
fn evaluate_routing(input: &GuardInput) -> GuardVerdict {
    let (true, Some(command), Some(project)) =
        (input.workspace, &input.command, &input.project_dir)
    else {
        return GuardVerdict::Allow(None);
    };
    match workspace_search_refusal(command, project) {
        Some(reason) => GuardVerdict::Block(reason),
        None => GuardVerdict::Allow(None),
    }
}

/// The most recent result of each gate, in first-seen order. The ship trace
/// keeps every run; a gate that failed and was then re-run green is green.
fn latest_check_per_gate(checks: &[CheckSummary]) -> Vec<&CheckSummary> {
    let mut latest: Vec<&CheckSummary> = Vec::new();
    for check in checks {
        match latest.iter_mut().find(|c| c.name == check.name) {
            Some(slot) => *slot = check,
            None => latest.push(check),
        }
    }
    latest
}

/// Does this shell command line run `git commit` in any of its segments?
pub(crate) fn is_git_commit(command: &str) -> bool {
    command
        .split([';', '|', '\n'])
        .flat_map(|seg| seg.split("&&"))
        .any(|seg| {
            let mut words = seg.split_whitespace();
            // Skip leading env assignments (`FOO=bar git commit`).
            let mut head = words.next();
            while head.is_some_and(|w| w.contains('=') && !w.starts_with('-')) {
                head = words.next();
            }
            let is_git = head.is_some_and(|w| w == "git" || w.ends_with("/git"));
            is_git && words.any(|w| w == "commit")
        })
}

#[cfg(test)]
#[path = "guard_decision.tests.rs"]
mod tests;
