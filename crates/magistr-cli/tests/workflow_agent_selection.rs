//! Native selection commands run against isolated preferences without provider inference.

use magistr::config::session_selection::SessionSelection;
use magistr::workflow::iteration::IterationLoop;
use std::path::Path;
use std::process::{Command, Output};

fn command(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_magistr"))
        .args(["--no-tui"])
        .args(args)
        .current_dir(root)
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("personal"))
        .env_remove("MAGISTR_AGENT_BACKEND")
        .env_remove("MAGISTR_CONTINUE")
        .output()
        .unwrap()
}

fn succeeds(root: &Path, args: &[&str]) -> Output {
    let output = command(root, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn preview(root: &Path, args: &[&str]) -> serde_json::Value {
    serde_json::from_slice(&succeeds(root, args).stdout).unwrap()
}

#[test]
fn native_previews_share_options_and_never_start_work() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "next",
            "--agent",
            "codex",
            "--model",
            "chosen",
            "--effort",
            "high",
            "--show-config",
        ],
        vec![
            "next",
            "--headless",
            "--format",
            "json",
            "--agent",
            "codex",
            "--model",
            "chosen",
            "--effort",
            "high",
            "--show-config",
        ],
        vec![
            "next-loop",
            "--agent",
            "codex",
            "--model",
            "chosen",
            "--effort",
            "high",
            "--show-config",
        ],
        vec![
            "run",
            "unused prompt",
            "--provider",
            "codex",
            "-m",
            "chosen",
            "--effort",
            "high",
            "--show-config",
        ],
    ] {
        let selected = preview(dir.path(), &args);
        assert_eq!(selected["agent"], "codex");
        assert_eq!(selected["model"], "chosen");
        assert_eq!(selected["effort"], "high");
        assert_eq!(selected["sources"]["provider"], "command option");
        assert!(!dir.path().join(".magistr/loop-state.json").exists());
        assert!(!dir.path().join(".workbench").exists());
    }
}

#[test]
fn saved_preferences_and_overrides_have_the_same_precedence_in_the_real_cli() {
    let dir = tempfile::tempdir().unwrap();
    succeeds(dir.path(), &["config", "set", "agent", "claude"]);
    succeeds(
        dir.path(),
        &[
            "config",
            "set",
            "model",
            "personal-model",
            "--agent",
            "codex",
        ],
    );
    succeeds(
        dir.path(),
        &["config", "set", "agent", "codex", "--project"],
    );
    succeeds(
        dir.path(),
        &["config", "set", "model", "project-model", "--project"],
    );
    succeeds(
        dir.path(),
        &["config", "set", "effort", "high", "--project"],
    );
    let selected = preview(dir.path(), &["next", "--show-config"]);
    assert_eq!(selected["agent"], "codex");
    assert_eq!(selected["model"], "project-model");
    assert_eq!(selected["effort"], "high");
    assert_eq!(selected["sources"]["model"], "project settings");
    let overridden = preview(
        dir.path(),
        &["next", "--show-config", "--model", "flag-model"],
    );
    assert_eq!(overridden["model"], "flag-model");
    assert_eq!(overridden["sources"]["model"], "command option");
    let defaults = preview(
        dir.path(),
        &[
            "next",
            "--show-config",
            "--model",
            "default",
            "--effort",
            "default",
        ],
    );
    assert!(defaults["model"].is_null());
    assert!(defaults["effort"].is_null());
    let other = preview(
        dir.path(),
        &["run", "unused prompt", "--show-config", "--agent", "openai"],
    );
    assert!(
        other["model"].is_null(),
        "the Codex model cannot cross to another agent"
    );
    assert!(other["effort"].is_null());
}

#[test]
fn loop_preview_recovers_selection_without_mutating_the_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".magistr")).unwrap();
    let spec = dir.path().join("items.md");
    std::fs::write(&spec, "- [ ] one\n").unwrap();
    let mut running = IterationLoop::with_max(dir.path(), &spec, 3).unwrap();
    running
        .bind_session_selection(SessionSelection {
            agent: "codex".into(),
            model: Some("saved-model".into()),
            effort: Some("high".into()),
            sources: Default::default(),
        })
        .unwrap();
    running.persist().unwrap();
    std::fs::write(
        dir.path().join(".magistr/agents.toml"),
        "malformed preference file",
    )
    .unwrap();
    let path = dir.path().join(".magistr/loop-state.json");
    let before = std::fs::read(&path).unwrap();
    let selected = preview(dir.path(), &["next-loop", "--show-config"]);
    assert_eq!(selected["model"], "saved-model");
    let refused = command(
        dir.path(),
        &["next-loop", "--show-config", "--model", "changed"],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("saved agent settings"));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn unknown_agent_and_unused_agent_options_are_refused_before_work() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["next", "--agent", "unknown-agent", "--show-config"],
        vec!["next", "--gate-only", "--model", "chosen"],
    ] {
        assert!(!command(dir.path(), &args).status.success());
    }
    assert!(!dir.path().join(".magistr/loop-state.json").exists());
}
#[test]
fn paused_runs_keep_their_selection_and_settled_runs_can_choose_again() {
    use magistr_engine::workflow::StopReason;
    for reason in [StopReason::Paused, StopReason::Cancelled] {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("items.md");
        std::fs::write(&spec, "- [ ] one\n").unwrap();
        let mut previous = IterationLoop::with_max(dir.path(), &spec, 3).unwrap();
        previous
            .bind_session_selection(SessionSelection {
                agent: "codex".into(),
                model: Some("old-model".into()),
                effort: None,
                sources: Default::default(),
            })
            .unwrap();
        previous.stop(reason);
        previous.persist().unwrap();
        let args = [
            "next-loop",
            "--agent",
            "codex",
            "--model",
            "new-model",
            "--show-config",
        ];
        if reason == StopReason::Paused {
            assert!(!command(dir.path(), &args).status.success());
            assert_eq!(
                preview(dir.path(), &["next-loop", "--show-config"])["model"],
                "old-model"
            );
        } else {
            let selected = preview(dir.path(), &args);
            assert_eq!(selected["model"], "new-model");
            let mut fresh = IterationLoop::start_or_recover(dir.path(), &spec, 5, None).unwrap();
            assert!(fresh.state().session_selection.is_none());
            fresh
                .bind_session_selection(serde_json::from_value(selected).unwrap())
                .unwrap();
            assert_eq!(
                fresh
                    .state()
                    .session_selection
                    .as_ref()
                    .unwrap()
                    .model
                    .as_deref(),
                Some("new-model")
            );
            assert!(dir.path().join(".magistr/loop-history").is_dir());
        }
    }
}
