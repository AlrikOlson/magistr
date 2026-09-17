//! The doctor's `operations` section: the run checkpoint's operation
//! ledger, read as it lies on disk and correlated with what the process
//! table says right now. It is an observation of the ledger — the run's
//! own record of what it dispatched and saw — never a second authority on
//! any effect: reconciliation, not the doctor, asks the native store, the
//! receipt or Git what an effect came to, and only a loop start performs
//! it. Nothing here marks, reconciles or writes.

use std::path::Path;

use magistr_core::workflow::{
    iteration::{IterationState, OperationKind, OperationRecord},
    process_alive::is_process_alive,
    runtime_handle::RuntimeHandle,
};
use magistr_engine::workflow::StopReason;
use magistr_workspace::diagnostics::{
    DoctorDataClass, DoctorFinding, DoctorSection, DoctorSectionId,
};
use serde_json::{Value, json};

const SOURCE: &str = ".magistr/loop-state.json (read as it lies on disk; nothing is marked, reconciled or written) + the process table for the loop process and each recorded runtime handle";

/// How one unsettled record stands, given what the process table says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettlementState {
    /// A settled outcome: nothing is open.
    Settled,
    /// Unsettled and owned by a loop process that is still running.
    Live,
    /// The loop is gone but the runtime it dispatched is still alive: the
    /// next loop start waits for it and admits nothing meanwhile.
    Stalled,
    /// The loop and any runtime are gone: the next loop start reconciles
    /// it against the authority on its effect.
    Recoverable,
    /// Nothing can establish it: no runtime handle, or a host that cannot
    /// say whether a process lives.
    Indeterminate,
}

impl SettlementState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::Live => "live",
            Self::Stalled => "stalled",
            Self::Recoverable => "recoverable",
            Self::Indeterminate => "indeterminate",
        }
    }
}

/// One correlated row of the ledger plus the state it is in.
struct Row {
    detail: Value,
    state: SettlementState,
    runtime_pid: Option<u32>,
}

/// Observe the run checkpoint under `root`, the workspace whose durable
/// generation (if any) is `workspace_generation`.
pub(crate) fn operations_section(root: &Path, workspace_generation: Option<u64>) -> DoctorSection {
    let checkpoint = root.join(".magistr").join("loop-state.json");
    let mut findings = Vec::new();
    let detail = match IterationState::read(root) {
        Ok(None) => {
            findings.push(DoctorFinding::info(
                "loop-state.json",
                DoctorDataClass::NotApplicable,
                "no run checkpoint: no loop run is recorded for this root",
            ));
            json!({ "checkpoint": checkpoint.display().to_string(), "present": false })
        }
        Err(error) => {
            findings.push(DoctorFinding::unhealthy(
                "loop-state.json",
                DoctorDataClass::Durable,
                format!("the run checkpoint cannot be read: {error}"),
                "preserve or remove .magistr/loop-state.json explicitly to start a new run; a torn or foreign checkpoint is never guessed at",
            ));
            json!({ "checkpoint": checkpoint.display().to_string(), "present": true, "readable": false })
        }
        Ok(Some(state)) => {
            let loop_pid = state.run_process();
            let loop_alive = loop_pid.and_then(is_process_alive);
            let rows: Vec<Row> = state
                .operations
                .records()
                .iter()
                .map(|record| row(&state, record, loop_alive))
                .collect();
            summarize(&state, loop_pid, loop_alive, &rows, &mut findings);
            json!({
                "checkpoint": checkpoint.display().to_string(),
                "present": true,
                "readable": true,
                "project": state.run_id.project().as_str(),
                "run": state.run_id.local(),
                "workspace": { "root": root.display().to_string(), "generation": workspace_generation },
                "loopProcess": {
                    "pid": loop_pid,
                    "alive": loop_alive,
                    "evidence": "the pid the run key names; not pinned to a start identity, so a live answer may be a reused id",
                },
                "runState": {
                    "mode": state.run.mode,
                    "completedAttempts": state.run.completed_attempts,
                    "maxIterations": state.run.max_iterations,
                    "consecutiveNoCommits": state.run.consecutive_no_commits,
                    "terminalReason": state.run.terminal_reason,
                    "lastSettledAttempt": state.run.last_settled_attempt,
                    "attemptStarted": state.attempt_started,
                    "lastCommitSha": state.last_commit_sha,
                    "nativeChunk": state.native_chunk_id,
                },
                "identities": {
                    "project": "the run key's project, above",
                    "workspace": "the canonical root and its durable generation, above",
                    "client": "live clients are the owner section's connections; the checkpoint records none",
                    "session": "not persisted: a provider session and its turns exist only in the live host's session feed and end with it",
                    "turn": "not persisted: see session",
                    "run": "the run key, above",
                    "attempt": "each row's command and attempt number",
                    "check": "each receipt row's gate, task and receipt id",
                    "source": "the workspace generation above; a source snapshot names the same counter",
                },
                "records": rows.iter().map(|row| row.detail.clone()).collect::<Vec<_>>(),
                "retention": "this section is a diagnostic: it is computed on request and kept nowhere; the checkpoint and the receipts it names are the evidence",
            })
        }
    };
    DoctorSection {
        id: DoctorSectionId::Operations,
        source: SOURCE.to_owned(),
        findings,
        detail,
    }
}

fn row(state: &IterationState, record: &OperationRecord, loop_alive: Option<bool>) -> Row {
    let settled = record.is_settled();
    let handle = match record.kind {
        OperationKind::ProviderDispatch => {
            record.external.as_deref().and_then(RuntimeHandle::parse)
        }
        _ => None,
    };
    let runtime_live = handle.as_ref().and_then(RuntimeHandle::is_live);
    let (state_of, explanation) = classify(record, loop_alive, handle.as_ref(), runtime_live);
    let cancellation_requested = matches!(
        state.run.terminal_reason,
        Some(StopReason::Cancelled | StopReason::Aborted)
    );
    let ownership_released = match record.kind {
        OperationKind::ProviderDispatch => match (handle.as_ref(), runtime_live) {
            (Some(_), Some(live)) => Some(!live),
            _ => None,
        },
        _ => loop_alive.map(|alive| !alive),
    };
    let check = (record.kind == OperationKind::Receipt).then(|| {
        let (gate, task) = record
            .target
            .strip_prefix("gate:")
            .and_then(|rest| rest.split_once("@task:"))
            .unzip();
        json!({
            "gate": gate,
            "task": task,
            "receipt": record.external.as_deref().and_then(|handle| handle.strip_prefix("receipt:")),
        })
    });
    let runtime = handle.as_ref().map(|handle| {
        json!({
            "handle": handle.to_string(),
            "pid": handle.pid(),
            "start": handle.start(),
            "live": runtime_live,
        })
    });
    let detail = json!({
        "project": record.attempt.command().run().project().as_str(),
        "run": record.attempt.command().run().local(),
        "attempt": {
            "command": record.attempt.command().local(),
            "number": record.attempt.number().get(),
        },
        "kind": record.kind,
        "target": record.target,
        "external": record.external,
        "check": check,
        "runtime": runtime,
        "outcome": record.outcome,
        "settlement": {
            "cancellationRequested": cancellation_requested,
            "terminalObserved": settled,
            "effectsSettled": settled,
            "ownershipReleased": ownership_released,
        },
        "state": state_of.as_str(),
        "explanation": explanation,
    });
    Row {
        detail,
        state: state_of,
        runtime_pid: handle.map(|handle| handle.pid()),
    }
}

/// Where an unsettled record stands and why, in the words recovery uses.
fn classify(
    record: &OperationRecord,
    loop_alive: Option<bool>,
    handle: Option<&RuntimeHandle>,
    runtime_live: Option<bool>,
) -> (SettlementState, String) {
    if record.is_settled() {
        return (
            SettlementState::Settled,
            format!("{}: the outcome is established and latched", record.outcome),
        );
    }
    let authority = match record.kind {
        OperationKind::Reservation => "the checkpoint that carries it (completed)",
        OperationKind::NativeMutation => "the chunk as the native store holds it now",
        OperationKind::Receipt => "the durable receipt the record names, never a same-named check",
        OperationKind::Publication => "nothing: it is cancelled and attached surfaces reseed",
        OperationKind::ProviderDispatch => "the attempt's Git baseline once the runtime is gone",
    };
    match loop_alive {
        Some(true) => (
            SettlementState::Live,
            format!(
                "{}: in flight under the running loop process, which settles it",
                record.outcome
            ),
        ),
        None => (
            SettlementState::Indeterminate,
            format!(
                "{}: this host cannot say whether the loop process lives",
                record.outcome
            ),
        ),
        Some(false) => match record.kind {
            OperationKind::ProviderDispatch => match (handle, runtime_live) {
                (None, _) => (
                    SettlementState::Indeterminate,
                    format!(
                        "{}: the loop is gone and the dispatch has no runtime handle, so nothing says whether a runtime exists or existed; it stays open until the checkpoint is set aside by hand",
                        record.outcome
                    ),
                ),
                (Some(handle), Some(true)) => (
                    SettlementState::Stalled,
                    format!(
                        "{}: the loop is gone but runtime {handle} is still alive; the next loop start waits for it (unreconciled) and admits nothing meanwhile",
                        record.outcome
                    ),
                ),
                (Some(handle), Some(false)) => (
                    SettlementState::Recoverable,
                    format!(
                        "{}: the loop and runtime {handle} are gone; the next loop start establishes completed or failed from {authority}",
                        record.outcome
                    ),
                ),
                (Some(handle), None) => (
                    SettlementState::Indeterminate,
                    format!(
                        "{}: this host cannot say whether runtime {handle} lives",
                        record.outcome
                    ),
                ),
            },
            _ => (
                SettlementState::Recoverable,
                format!(
                    "{}: the loop is gone; the next loop start reconciles it against {authority}",
                    record.outcome
                ),
            ),
        },
    }
}

fn summarize(
    state: &IterationState,
    loop_pid: Option<u32>,
    loop_alive: Option<bool>,
    rows: &[Row],
    findings: &mut Vec<DoctorFinding>,
) {
    let count = |wanted: SettlementState| rows.iter().filter(|row| row.state == wanted).count();
    let settled = count(SettlementState::Settled);
    let run = state.run_id.local();
    let loop_process = match (loop_pid, loop_alive) {
        (Some(pid), Some(true)) => format!("loop process {pid} alive (pid evidence only)"),
        (Some(pid), Some(false)) => format!("loop process {pid} gone"),
        (Some(pid), None) => format!("loop process {pid} liveness unknown"),
        (None, _) => "loop process not named by the run key".to_owned(),
    };
    let stop = state.run.terminal_reason.map_or_else(
        || "no stop latched".to_owned(),
        |reason| format!("stop latched: {}", reason.as_str()),
    );
    findings.push(DoctorFinding::healthy(
        "operations",
        format!(
            "run {run}: {} record(s), {settled} settled, {} live, {} stalled, {} recoverable, {} indeterminate; {loop_process}; {stop}",
            rows.len(),
            count(SettlementState::Live),
            count(SettlementState::Stalled),
            count(SettlementState::Recoverable),
            count(SettlementState::Indeterminate),
        ),
    ));
    for row in rows {
        match row.state {
            SettlementState::Stalled => findings.push(DoctorFinding::unhealthy(
                "operations",
                DoctorDataClass::Durable,
                format!(
                    "stalled: attempt {} ({}) is owned by runtime pid {} which outlived the loop",
                    row.detail["attempt"]["command"],
                    row.detail["kind"],
                    row.runtime_pid.unwrap_or(0)
                ),
                "wait for that runtime to end, or end it; the next loop start then reconciles the attempt from its Git baseline. Nothing is signalled or replayed for you",
            )),
            SettlementState::Indeterminate => findings.push(DoctorFinding::unhealthy(
                "operations",
                DoctorDataClass::Durable,
                format!(
                    "indeterminate: attempt {} ({}) cannot be established from here",
                    row.detail["attempt"]["command"], row.detail["kind"]
                ),
                "a loop start reconciles what it can; an attempt with no runtime handle stays open until .magistr/loop-state.json is set aside explicitly",
            )),
            SettlementState::Recoverable => findings.push(DoctorFinding::info(
                "operations",
                DoctorDataClass::Durable,
                format!(
                    "recoverable: attempt {} ({}) — {}",
                    row.detail["attempt"]["command"], row.detail["kind"], row.detail["explanation"]
                ),
            )),
            SettlementState::Live | SettlementState::Settled => {}
        }
    }
}

#[cfg(test)]
#[path = "doctor_operations.tests.rs"]
mod tests;
