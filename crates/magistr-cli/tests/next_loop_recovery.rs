//! Exercise the real loop command handlers with isolated saved progress.
//! The current streamed and machine-readable commands preserve admitted run state.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use magistr::agent::Backend;
use magistr::cli::HeadlessFormat;
use magistr::commands::next::{cmd_next_loop, cmd_next_loop_headless};
use magistr::config::inputs::ProjectInputs;
use magistr::permissions::PermissionProfile;
use magistr_core::workflow::iteration::{
    IterationLoop, IterationState, OperationKind, OperationOutcome, PreCheckResult,
};
use magistr_engine::workflow::StopReason;

const ENTRIES: [&str; 2] = ["interactive", "headless"];

fn selection() -> magistr::config::session_selection::SessionSelection {
    magistr::config::session_selection::SessionSelection {
        agent: "codex".into(),
        model: None,
        effort: None,
        sources: Default::default(),
    }
}

fn stopped_dispatch(dir: &Path) -> IterationState {
    previous(dir, None, true);
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(checkpoint(dir)).unwrap()).unwrap();
    // A stopped owner with a positive PID outside supported hosts' PID ranges.
    let run = "run-1-2000000000";
    json["run_id"]["local"] = run.into();
    for record in json["operations"]["records"].as_array_mut().unwrap() {
        record["attempt"]["command"]["run"]["local"] = run.into();
    }
    std::fs::write(checkpoint(dir), serde_json::to_vec(&json).unwrap()).unwrap();
    saved(dir)
}

fn resolve_command(dir: &Path, state: &IterationState, outcome: &str) -> std::process::Command {
    let dispatch = state
        .operations
        .records()
        .iter()
        .find(|record| record.kind == OperationKind::ProviderDispatch)
        .unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_magistr"));
    command
        .args(["resolve-dispatch"])
        .arg(dir)
        .args([
            "--run",
            state.run_id.local(),
            "--command",
            dispatch.attempt.command().local(),
            "--attempt",
            &dispatch.attempt.number().to_string(),
            "--outcome",
            outcome,
            "--reason",
            "Checked the agent has stopped and reviewed its work.",
        ])
        .env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .current_dir(dir);
    command
}

#[test]
fn resolve_dispatch_cli_preserves_saved_work_and_records_each_final_outcome() {
    for outcome in ["failed", "cancelled", "completed"] {
        let dir = project();
        let before = stopped_dispatch(dir.path());
        let output = resolve_command(dir.path(), &before, outcome)
            .arg("--confirm-stopped")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let after = saved(dir.path());
        let dispatch = after
            .operations
            .records()
            .iter()
            .find(|record| record.kind == OperationKind::ProviderDispatch)
            .unwrap();
        assert_eq!(dispatch.outcome.as_str(), outcome);
        assert!(dispatch.resolution.as_deref().unwrap().contains("reviewed"));
        assert_eq!(after.run, before.run);
        assert_eq!(after.run_id, before.run_id);
        assert_eq!(after.last_commit_sha, before.last_commit_sha);
        assert_eq!(
            after.operations.records()[0],
            before.operations.records()[0]
        );
        assert_context_kept(dir.path());
        let mut recovered =
            IterationLoop::open_or_recover(dir.path(), dir.path().join("spec.md"), 9, None)
                .unwrap();
        let retry = recovered
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
            .unwrap();
        if outcome == "completed" {
            assert!(retry.is_none());
        } else {
            assert_eq!(retry.unwrap().number().get(), 2);
        }
        let settled = std::fs::read(checkpoint(dir.path())).unwrap();
        assert!(
            !resolve_command(dir.path(), &before, outcome)
                .arg("--confirm-stopped")
                .output()
                .unwrap()
                .status
                .success()
        );
        assert_eq!(std::fs::read(checkpoint(dir.path())).unwrap(), settled);
    }
}

#[test]
fn resolve_dispatch_cli_refuses_missing_confirmation_stale_ids_and_live_work() {
    let dir = project();
    let state = stopped_dispatch(dir.path());
    let original = std::fs::read(checkpoint(dir.path())).unwrap();
    let output = resolve_command(dir.path(), &state, "failed")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let mut stale = state.clone();
    stale.run_id = serde_json::from_value(serde_json::json!({
        "project": state.run_id.project().as_str(), "local": "wrong-run"
    }))
    .unwrap();
    assert!(
        !resolve_command(dir.path(), &stale, "failed")
            .arg("--confirm-stopped")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(std::fs::read(checkpoint(dir.path())).unwrap(), original);
    let lease = magistr_core::workflow::loop_lease::LoopLease::acquire(dir.path()).unwrap();
    assert!(
        !resolve_command(dir.path(), &state, "failed")
            .arg("--confirm-stopped")
            .output()
            .unwrap()
            .status
            .success()
    );
    drop(lease);
    let mut json = serde_json::to_value(&state).unwrap();
    let dispatch = json["operations"]["records"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| record["kind"] == "provider_dispatch")
        .unwrap();
    dispatch["external"] = format!("pid:{}", std::process::id()).into();
    std::fs::write(checkpoint(dir.path()), serde_json::to_vec(&json).unwrap()).unwrap();
    let live = std::fs::read(checkpoint(dir.path())).unwrap();
    let output = resolve_command(dir.path(), &state, "failed")
        .arg("--confirm-stopped")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("agent is still running"));
    assert_eq!(std::fs::read(checkpoint(dir.path())).unwrap(), live);
    previous(dir.path(), None, true);
    let live_owner = saved(dir.path());
    let output = resolve_command(dir.path(), &live_owner, "failed")
        .arg("--confirm-stopped")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("process is still running"));
}

#[tokio::test]
async fn every_loop_entry_refuses_a_concurrent_checkpoint_owner() {
    for entry in ENTRIES {
        let dir = project();
        let _lease = magistr_core::workflow::loop_lease::LoopLease::acquire(dir.path()).unwrap();
        let result = invoke(entry, dir.path()).await;
        assert!(result.contains("already running"), "{entry}: {result}");
        assert!(!checkpoint(dir.path()).exists());
        assert_context_kept(dir.path());
    }
}

const PLAN: &str = "Continue the saved implementation plan.\n";
const TASK: &str = "Finish the interrupted task.\n";

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".magistr")).unwrap();
    let config = serde_json::json!({
        "project": {"name": "loop-recovery"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "loop-recovery",
        "roadmap_backend": "yaml",
        "spec": {"file": "spec.md", "unchecked_pattern": "^\\s*- \\[ \\]"}
    });
    std::fs::write(
        dir.path().join(".magistr/project.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.path().join("spec.md"), "- [ ] saved work\n").unwrap();
    let native = context_inputs(dir.path()).workflow().clone();
    let writer = magistr_engine::ProjectAuthority::admit(&native, magistr_engine::WriterRole::Host)
        .unwrap()
        .open();
    writer
        .ship_set_objective(PLAN.into(), vec![], vec![], "saved-work".into())
        .unwrap();
    writer
        .ship_add_task(
            "saved-task".into(),
            TASK.into(),
            magistr_engine::TaskType::Implement,
            None,
        )
        .unwrap();
    writer.ship_start_task("saved-task").unwrap();
    dir
}

fn checkpoint(dir: &Path) -> PathBuf {
    dir.join(".magistr/loop-state.json")
}

fn saved(dir: &Path) -> IterationState {
    serde_json::from_slice(&std::fs::read(checkpoint(dir)).unwrap()).unwrap()
}

fn context_inputs(dir: &Path) -> ProjectInputs {
    ProjectInputs::from_lookup(dir, |key| {
        (key == "THINK_AND_SHIP_DATA_DIR").then(|| dir.join("workflow-data").display().to_string())
    })
}

fn assert_context_kept(dir: &Path) {
    let reader = magistr_engine::NativeReader::open(context_inputs(dir).workflow());
    let run = reader.ship_snapshot().unwrap().current.unwrap();
    assert_eq!(run.objective.description, PLAN);
    assert_eq!(run.tasks.len(), 1);
    assert_eq!(run.tasks[0].title, TASK);
    assert_eq!(
        run.tasks[0].status,
        magistr_engine::native::TaskStatus::Active
    );
}

fn previous(dir: &Path, stop: Option<StopReason>, unresolved: bool) -> IterationLoop {
    let mut run = IterationLoop::open(dir, dir.join("spec.md"), 3, None).unwrap();
    run.bind_session_selection(selection()).unwrap();
    assert_eq!(run.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    run.set_budget_limit(Some(2.0)).unwrap();
    run.observe_budget(0.5);
    let dispatch = run
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
        .unwrap()
        .unwrap();
    run.start_operation(&dispatch, None).unwrap();
    if !unresolved {
        run.settle_operation(&dispatch, OperationOutcome::Completed)
            .unwrap();
        run.advance();
    }
    if let Some(stop) = stop {
        run.stop(stop);
    }
    run.persist().unwrap();
    run
}

async fn invoke(entry: &str, dir: &Path) -> String {
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "entry_fixture_child", "--nocapture"])
        .env("MAGISTR_RECOVERY_FIXTURE", dir)
        .env("MAGISTR_RECOVERY_ENTRY", entry)
        .env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("PATH", "/usr/bin:/bin")
        .current_dir(dir)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .expect("loop startup must finish without an agent")
        .unwrap();
    let report = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{entry}: {report}");
    let result: String =
        serde_json::from_slice(&std::fs::read(dir.join("entry-result.json")).expect(&report))
            .unwrap();
    result
}

#[tokio::test]
async fn entry_fixture_child() {
    let Some(dir) = std::env::var_os("MAGISTR_RECOVERY_FIXTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let entry = std::env::var("MAGISTR_RECOVERY_ENTRY").unwrap();
    // The command preflight has selected settings; these startup scenarios never launch an agent.
    let inputs = context_inputs(&dir).with_session_selection(selection());
    let profile = PermissionProfile::Cautious;
    let result = match entry.as_str() {
        "interactive" => {
            cmd_next_loop(
                &dir,
                magistr::commands::next::LoopRequest {
                    max_iterations: 8,
                    track: None,
                },
                Backend::Acp,
                profile,
                &inputs,
            )
            .await
        }
        "headless" => {
            cmd_next_loop_headless(
                &dir,
                magistr::commands::next::LoopRequest {
                    max_iterations: 8,
                    track: None,
                },
                Backend::Acp,
                HeadlessFormat::Json,
                None,
                profile,
                &inputs,
            )
            .await
        }
        _ => panic!("unknown fixture entry"),
    };
    let result = result.map_or_else(|error| format!("{error:#}"), |()| "ok".into());
    std::fs::write(
        dir.join("entry-result.json"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn settled_cancellation_archives_and_starts_with_the_requested_limit() {
    for entry in ENTRIES {
        let dir = project();
        let previous = previous(dir.path(), Some(StopReason::Cancelled), false);
        // Nothing can dispatch: the fixture's work is already complete.
        std::fs::write(dir.path().join("spec.md"), "- [x] saved work\n").unwrap();
        let result = invoke(entry, dir.path()).await;
        assert_eq!(result, "ok", "{entry}");
        let fresh = saved(dir.path());
        assert_ne!(fresh.run_id, previous.state().run_id, "{entry}");
        assert_eq!(fresh.run.max_iterations, 8, "{entry}");
        assert_eq!(fresh.run.budget_total_usd, 0.0, "{entry}");
        assert!(fresh.operations.records().is_empty(), "{entry}");
        let history: Vec<_> = std::fs::read_dir(dir.path().join(".magistr/loop-history"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(history.len(), 1, "{entry}");
        let archived: IterationState =
            serde_json::from_slice(&std::fs::read(&history[0]).unwrap()).unwrap();
        assert_eq!(&archived, previous.state(), "{entry}");
        // Cancellation settles the run, not necessarily its saved task.
        assert_context_kept(dir.path());
    }
}

#[tokio::test]
async fn paused_and_unfinished_runs_keep_identity_limits_and_context() {
    for entry in ENTRIES {
        for stop in [None, Some(StopReason::Paused)] {
            let dir = project();
            let previous = previous(dir.path(), stop, false);
            // Finish the spec externally so the non-paused command cannot launch.
            std::fs::write(dir.path().join("spec.md"), "- [x] saved work\n").unwrap();
            let result = invoke(entry, dir.path()).await;
            assert_eq!(result, "ok", "{entry}");
            let recovered = saved(dir.path());
            assert_eq!(recovered.run_id, previous.state().run_id, "{entry}");
            assert_eq!(recovered.run.max_iterations, 3, "{entry}");
            assert_eq!(recovered.run.budget_limit_usd, Some(2.0), "{entry}");
            assert_eq!(recovered.run.budget_total_usd, 0.5, "{entry}");
            assert_eq!(recovered.operations, previous.state().operations, "{entry}");
            if stop.is_some() {
                assert_eq!(&recovered, previous.state(), "{entry}");
            }
            assert!(
                !dir.path().join(".magistr/loop-history").exists(),
                "{entry}"
            );
            assert_context_kept(dir.path());
        }
    }
}

#[tokio::test]
async fn invalid_and_unresolved_checkpoints_refuse_without_discarding_context() {
    for entry in ENTRIES {
        for malformed in [false, true] {
            let dir = project();
            previous(dir.path(), Some(StopReason::Cancelled), true);
            if malformed {
                std::fs::write(checkpoint(dir.path()), "{invalid checkpoint").unwrap();
            }
            let before = std::fs::read(checkpoint(dir.path())).unwrap();
            let result = invoke(entry, dir.path()).await;
            if malformed {
                assert!(
                    result.contains("Cannot recover loop-state.json"),
                    "{entry}: {result}"
                );
            } else {
                assert!(
                    result.contains("outcome is not yet known"),
                    "{entry}: {result}"
                );
            }
            assert_eq!(
                std::fs::read(checkpoint(dir.path())).unwrap(),
                before,
                "{entry}"
            );
            assert!(
                !dir.path().join(".magistr/loop-history").exists(),
                "{entry}"
            );
            assert_context_kept(dir.path());
        }
    }
}
