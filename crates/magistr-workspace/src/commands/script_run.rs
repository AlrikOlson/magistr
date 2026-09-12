//! The script verb: typed Code Mode for the long tail (§9.7). The source runs
//! as a function body in the QuickJS-ng guest under the shared sandbox
//! policy; writes only QUEUE and resolve through the same confirm gate and
//! transaction pipeline every other verb uses. Sandbox kills map onto the
//! taxonomy; a guest that merely throws is data.
use super::{
    ScriptInput, ScriptInterpreter, ScriptResult, ScriptRow, ScriptStatus, WorkspaceCommands,
    gate::{self, GateInput},
    resolution::Resolution,
    script_guest::{GuestLoop, GuestRun},
    script_loop, script_queue,
};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    extensions::{ExtensionLane, PluginGuestPolicy},
    query::budget,
    transaction::FormatMode,
};

/// Ceiling on the return-value JSON echoed into CONFIRM_REQUIRED notes.
const CONFIRM_VALUE_CHARS: usize = 2000;

pub(super) async fn run(
    commands: &mut WorkspaceCommands<'_>,
    input: &ScriptInput,
    interpreter: &ScriptInterpreter,
    lane: &dyn ExtensionLane,
) -> WorkspaceResponse<ScriptRow> {
    if input.source.is_empty() {
        return budget::failure(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            "script: source must be a non-empty string",
        ));
    }
    let intent = match serde_json::to_vec(input) {
        Ok(intent) => intent,
        Err(error) => {
            return budget::failure(WorkspaceError::new(
                ErrorKind::PatternInvalid,
                error.to_string(),
            ));
        }
    };
    // Caps clamp DOWNWARD only: a request narrows the sandbox, never widens it.
    let policy = PluginGuestPolicy::clamped(input.fuel, input.memory_cap_bytes);
    let guest = script_loop::run(commands, input, interpreter, lane, policy).await;
    let stats = format!(
        "rounds {}, host calls {}, fuel used {}",
        guest.rounds, guest.host_calls, guest.fuel_used
    );
    let (done, stdout, stderr) = match guest.run {
        GuestRun::Error(error) => {
            return WorkspaceResponse::Failure {
                version: EnvelopeVersion::LATEST,
                error,
                notes: Some(vec![stats]),
            };
        }
        GuestRun::Threw {
            ref error,
            ref stdout,
            ref stderr,
        } => {
            let row = script_row(
                &guest,
                ScriptStatus::Threw,
                None,
                Some(error.clone()),
                stdout,
                stderr,
                0,
            );
            return success(
                vec![ScriptRow::Script(row)],
                input.budget,
                vec!["the script itself failed; nothing was queued or applied".into()],
            );
        }
        GuestRun::Done {
            ref done,
            ref stdout,
            ref stderr,
        } => (done, stdout, stderr),
    };
    let queued = done.edits.len() + done.files.len();
    if let Some(threw) = &done.threw {
        let row = script_row(
            &guest,
            ScriptStatus::Threw,
            None,
            Some(threw.clone()),
            stdout,
            stderr,
            queued,
        );
        return success(
            vec![ScriptRow::Script(row)],
            input.budget,
            vec!["the script threw; its queued writes were discarded, nothing was applied".into()],
        );
    }
    let row = script_row(
        &guest,
        ScriptStatus::Ok,
        Some(done.value.clone()),
        None,
        stdout,
        stderr,
        queued,
    );
    if queued == 0 {
        return success(vec![ScriptRow::Script(row)], input.budget, Vec::new());
    }
    let resolved = match script_queue::resolve(commands, done) {
        Ok(Resolution::Writes(resolved)) => resolved,
        // A queue whose effects evaporated with a presented confirm is exactly
        // what a stale confirm looks like (§7.7: fail closed).
        Ok(Resolution::NoOp(mut notes)) => {
            if input.confirm.is_some() {
                return budget::failure(WorkspaceError::new(
                    ErrorKind::ConfirmMismatch,
                    "confirm: the token was minted for a plan that no longer produces any change; re-run the call without confirm",
                ));
            }
            notes.push("nothing to change: no file content differs; no write was attempted".into());
            return success(vec![ScriptRow::Script(row)], input.budget, notes);
        }
        Ok(Resolution::Refused(error, notes)) => {
            return WorkspaceResponse::Failure {
                version: EnvelopeVersion::LATEST,
                error,
                notes: (!notes.is_empty()).then_some(notes),
            };
        }
        Err(error) => return budget::failure(error),
    };
    let value_note = format!("script return value: {}", truncated(&done.value));
    gated(commands, input, intent, resolved, row, value_note).await
}

/// Run the resolved queue through the shared gate and prepend the script row.
async fn gated(
    commands: &mut WorkspaceCommands<'_>,
    input: &ScriptInput,
    intent: Vec<u8>,
    resolved: super::resolved::ResolvedWrites,
    row: ScriptResult,
    value_note: String,
) -> WorkspaceResponse<ScriptRow> {
    let gated = gate::run(
        commands,
        GateInput {
            operation: "script",
            intent,
            confirm: input.confirm.as_deref(),
            dry_run: input.dry_run,
            auto_apply: input.auto_apply,
            budget: input.budget,
            format: FormatMode::Enabled,
        },
        resolved,
    )
    .await;
    match gated {
        WorkspaceResponse::Success { results, notes, .. } => {
            let mut rows = vec![ScriptRow::Script(row)];
            rows.extend(results.rows().iter().cloned().map(ScriptRow::Write));
            let mut notes = notes.unwrap_or_default();
            notes.push(if input.dry_run {
                "dryRun: the queued writes were previewed, not applied (§9.7 default)".into()
            } else {
                "queued writes applied through the shared write pipeline".into()
            });
            success(rows, input.budget, notes)
        }
        WorkspaceResponse::Failure {
            version,
            error,
            notes,
        } => {
            let mut notes = notes.unwrap_or_default();
            if matches!(error.kind, ErrorKind::ConfirmRequired { .. }) {
                notes.insert(0, value_note);
            }
            WorkspaceResponse::Failure {
                version,
                error,
                notes: Some(notes),
            }
        }
    }
}

fn truncated(value: &serde_json::Value) -> String {
    let json = value.to_string();
    if json.chars().count() > CONFIRM_VALUE_CHARS {
        format!(
            "{}…",
            json.chars().take(CONFIRM_VALUE_CHARS).collect::<String>()
        )
    } else {
        json
    }
}

fn script_row(
    guest: &GuestLoop,
    status: ScriptStatus,
    value: Option<serde_json::Value>,
    error: Option<String>,
    stdout: &str,
    stderr: &str,
    queued_writes: usize,
) -> ScriptResult {
    ScriptResult {
        status,
        value,
        error,
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
        fuel_used: guest.fuel_used,
        rounds: guest.rounds,
        host_calls: guest.host_calls,
        queued_writes,
    }
}

fn success(
    rows: Vec<ScriptRow>,
    requested: u64,
    notes: Vec<String>,
) -> WorkspaceResponse<ScriptRow> {
    budget::success(
        rows,
        requested,
        Freshness::Warm,
        notes,
        |row| match row {
            ScriptRow::Script(_) => "script".into(),
            ScriptRow::Write(write) => format!("path:{}", write.path),
        },
        vec!["raise budget".into()],
    )
}

#[cfg(test)]
#[path = "script_run.tests.rs"]
mod tests;
