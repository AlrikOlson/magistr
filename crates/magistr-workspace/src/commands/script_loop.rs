//! The re-entry loop: run, service the sentinel, re-run with the journal.
use serde_json::{Map, Value, json};

use super::{
    ScriptInput, ScriptInterpreter, WorkspaceCommands,
    script_guest::{GuestDone, GuestLoop, GuestRun},
    script_host_calls, script_program,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    extensions::{ExtensionLane, PluginGuestPolicy, WasiLaneOutcome},
    worker::WorkerGuestEnd,
};

/// Host-call ceiling: N calls cost N+1 guest rounds, so this bounds both.
pub(super) const HOST_CALLS_MAX: usize = 32;

/// What one settled round means for the loop.
enum Round {
    /// A host call was serviced and journaled; run the guest again.
    Again,
    Finished(GuestRun),
}

pub(super) async fn run(
    commands: &WorkspaceCommands<'_>,
    input: &ScriptInput,
    interpreter: &ScriptInterpreter,
    lane: &dyn ExtensionLane,
    policy: PluginGuestPolicy,
) -> GuestLoop {
    let program = script_program::build_guest_program(&input.source);
    let mut journal: Vec<Value> = Vec::new();
    let mut rounds = 0;
    let mut fuel_used = 0;
    let finish = |run: GuestRun, rounds: u32, journal: &[Value], fuel_used: u64| GuestLoop {
        run,
        rounds,
        host_calls: journal.len() as u32,
        fuel_used,
    };
    let nonce = match script_program::fresh_nonce() {
        Ok(nonce) => nonce,
        Err(error) => {
            let error = WorkspaceError::new(
                ErrorKind::EngineUnavailable,
                format!("script nonce entropy unavailable: {error}"),
            );
            return finish(GuestRun::Error(error), rounds, &journal, fuel_used);
        }
    };
    loop {
        if journal.len() >= HOST_CALLS_MAX {
            let error = WorkspaceError::new(
                ErrorKind::SandboxFuelExceeded,
                format!(
                    "script exceeded the host-call budget ({HOST_CALLS_MAX} workspace calls); batch reads (one search/glob instead of many) or split the work into smaller scripts"
                ),
            );
            return finish(GuestRun::Error(error), rounds, &journal, fuel_used);
        }
        rounds += 1;
        let argv = vec![
            "qjs".to_owned(),
            "-e".to_owned(),
            program.clone(),
            Value::Array(journal.clone()).to_string(),
            nonce.clone(),
        ];
        let out = match lane
            .run_wasi(interpreter.bytes().to_vec(), argv, policy)
            .await
        {
            Ok(out) => out,
            // Worker down, sandbox lane absent, or the request was rejected —
            // typed and retryable; the supervisor is already restarting the child.
            Err(error) => {
                let error = WorkspaceError::new(
                    ErrorKind::EngineUnavailable,
                    format!("script guest could not run: {}", error.message),
                );
                return finish(GuestRun::Error(error), rounds, &journal, fuel_used);
            }
        };
        fuel_used += out.fuel_used;
        match settle(commands, &out, &nonce, rounds, policy, &mut journal).await {
            Round::Again => continue,
            Round::Finished(run) => return finish(run, rounds, &journal, fuel_used),
        }
    }
}

/// Map one round's end onto the taxonomy or service its sentinel.
async fn settle(
    commands: &WorkspaceCommands<'_>,
    out: &WasiLaneOutcome,
    nonce: &str,
    rounds: u32,
    policy: PluginGuestPolicy,
    journal: &mut Vec<Value>,
) -> Round {
    let threw = |error: String| {
        Round::Finished(GuestRun::Threw {
            error,
            stdout: script_program::strip_sentinels(&out.stdout, nonce),
            stderr: out.stderr.clone(),
        })
    };
    let refused = |kind: ErrorKind, message: String| {
        Round::Finished(GuestRun::Error(WorkspaceError::new(kind, message)))
    };
    match &out.end {
        WorkerGuestEnd::OutOfFuel => refused(
            ErrorKind::SandboxFuelExceeded,
            format!(
                "script exhausted its fuel allowance ({} per round, round {rounds}); split the work into smaller scripts or reduce its scope",
                policy.fuel
            ),
        ),
        WorkerGuestEnd::MemoryCap => refused(
            ErrorKind::SandboxMemoryExceeded,
            format!(
                "script breached its {}-byte memory cap (round {rounds}); reduce the working set — stream over files instead of accumulating them",
                policy.memory_cap_bytes
            ),
        ),
        // The interpreter artifact itself misbehaved — an engine-side
        // problem, not the user's program: typed, retryable.
        WorkerGuestEnd::GuestTrap(detail)
        | WorkerGuestEnd::InvalidGuest(detail)
        | WorkerGuestEnd::CapDenied(detail) => refused(
            ErrorKind::EngineUnavailable,
            format!("script guest interpreter failed: {detail}"),
        ),
        WorkerGuestEnd::GuestExit(code) if *code != 0 => {
            let stderr = out.stderr.trim();
            threw(if stderr.is_empty() {
                format!("guest exited with code {code}")
            } else {
                stderr.to_owned()
            })
        }
        WorkerGuestEnd::Ok | WorkerGuestEnd::GuestExit(_) => {
            let Some(sentinel) = script_program::last_sentinel(&out.stdout, nonce) else {
                return threw(
                    "guest produced no result frame (the source may have escaped the wrapper)"
                        .into(),
                );
            };
            match sentinel.get("k").and_then(Value::as_str) {
                Some("nondet") => threw(format!(
                    "nondeterministic host-call replay at call #{}: the script's workspace calls must not depend on Date.now/Math.random or other per-run state",
                    sentinel.get("i").map_or("?".to_owned(), Value::to_string)
                )),
                Some("call") => service_call(commands, &sentinel, journal).await,
                Some("done") => Round::Finished(GuestRun::Done {
                    done: GuestDone {
                        value: sentinel.get("value").cloned().unwrap_or(Value::Null),
                        threw: sentinel
                            .get("threw")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        edits: list(sentinel.get("edits")),
                        files: list(sentinel.get("files")),
                    },
                    stdout: script_program::strip_sentinels(&out.stdout, nonce),
                    stderr: out.stderr.clone(),
                }),
                other => threw(format!(
                    "guest emitted an unknown result frame kind {}",
                    other.map_or("null".to_owned(), |k| format!("{k:?}"))
                )),
            }
        }
    }
}

/// Service one host call and journal it under the exact key the guest's
/// shim will compare on replay.
async fn service_call(
    commands: &WorkspaceCommands<'_>,
    sentinel: &Map<String, Value>,
    journal: &mut Vec<Value>,
) -> Round {
    let req = sentinel.get("req");
    let method = req.and_then(|req| req.get("method"));
    let args = req.and_then(|req| req.get("args"));
    match script_host_calls::service(commands, method, args).await {
        Ok(response) => {
            let key = json!({"method": method, "args": args.cloned().unwrap_or(Value::Null)});
            journal.push(json!({"k": key.to_string(), "r": response}));
            Round::Again
        }
        Err(denied) => Round::Finished(GuestRun::Error(WorkspaceError::new(
            ErrorKind::SandboxCapDenied,
            denied,
        ))),
    }
}

fn list(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}
