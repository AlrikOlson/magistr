use super::*;

fn check(required: bool, passed: bool, verified: bool) -> CheckSummary {
    named("gate", required, passed, verified)
}

fn named(name: &str, required: bool, passed: bool, verified: bool) -> CheckSummary {
    CheckSummary {
        name: name.into(),
        required,
        passed,
        verified,
    }
}

fn shipped() -> GuardInput {
    GuardInput {
        objective_status: Some("completed".into()),
        tasks_total: 3,
        checks: vec![check(true, true, true)],
        ..GuardInput::default()
    }
}

fn in_flight() -> GuardInput {
    GuardInput {
        objective_status: Some("active".into()),
        tasks_total: 3,
        ..GuardInput::default()
    }
}

// ── Stop ────────────────────────────────────────────────────────────

#[test]
fn stop_is_silent_while_the_objective_is_in_flight() {
    let input = GuardInput {
        chunk_in_progress: true,
        ..in_flight()
    };
    assert_eq!(
        evaluate(GuardEvent::Stop, &input),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn stop_is_silent_without_an_objective() {
    let input = GuardInput {
        ..GuardInput::default()
    };
    assert_eq!(
        evaluate(GuardEvent::Stop, &input),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn stop_allows_a_shipped_objective_with_a_clean_tree_and_closed_chunk() {
    assert_eq!(
        evaluate(GuardEvent::Stop, &shipped()),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn stop_allows_a_shipped_objective_without_claiming_ownership_of_worktree_changes() {
    assert_eq!(
        evaluate(GuardEvent::Stop, &shipped()),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn stop_blocks_a_shipped_objective_whose_chunk_is_still_in_progress() {
    let input = GuardInput {
        chunk_in_progress: true,
        ..shipped()
    };
    match evaluate(GuardEvent::Stop, &input) {
        GuardVerdict::Block(reason) => {
            assert!(reason.contains("roadmap item is still open"), "{reason}");
            assert!(reason.contains("roadmap_complete_chunk"), "{reason}");
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn stop_guidance_preserves_unrelated_changes_and_never_requests_exports() {
    let input = GuardInput {
        chunk_in_progress: true,
        ..shipped()
    };
    let GuardVerdict::Block(reason) = evaluate(GuardEvent::Stop, &input) else {
        panic!("the objective's own chunk must be closed");
    };
    assert!(reason.contains("Preserve unrelated changes"), "{reason}");
    assert!(!reason.contains("stage everything"), "{reason}");
    assert!(!reason.contains("ROADMAP"), "{reason}");
}

#[test]
fn stop_never_blocks_twice_in_one_turn() {
    let input = GuardInput {
        stop_hook_active: true,
        ..shipped()
    };
    assert!(matches!(
        evaluate(GuardEvent::Stop, &input),
        GuardVerdict::Allow(Some(_))
    ));
}

// ── Commit ──────────────────────────────────────────────────────────

#[test]
fn commit_ignores_bash_commands_that_are_not_git_commit() {
    for cmd in [
        "cargo nextest run",
        "git status",
        "git add -A && git log",
        "echo commit",
    ] {
        let input = GuardInput {
            command: Some(cmd.into()),
            checks: vec![check(true, false, true)],
            ..in_flight()
        };
        assert_eq!(
            evaluate(GuardEvent::Commit, &input),
            GuardVerdict::Allow(None),
            "{cmd}"
        );
    }
}

#[test]
fn commit_without_an_objective_is_not_gated() {
    let input = GuardInput {
        command: Some("git commit -m x".into()),
        ..GuardInput::default()
    };
    assert!(matches!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Allow(Some(_))
    ));
}

#[test]
fn commit_blocks_when_a_required_check_failed() {
    let input = GuardInput {
        command: Some("git add -A && git commit -m 'feat: x'".into()),
        checks: vec![check(true, true, true), check(true, false, true)],
        ..in_flight()
    };
    match evaluate(GuardEvent::Commit, &input) {
        GuardVerdict::Block(reason) => assert!(reason.contains("FAILED"), "{reason}"),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn commit_blocks_when_no_verified_check_exists_for_a_planned_objective() {
    let input = GuardInput {
        command: Some("git commit -q -F -".into()),
        checks: vec![check(true, true, false)], // self-reported only
        ..in_flight()
    };
    match evaluate(GuardEvent::Commit, &input) {
        GuardVerdict::Block(reason) => assert!(reason.contains("verified"), "{reason}"),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn commit_allows_after_a_verified_passing_required_check() {
    let input = GuardInput {
        command: Some("git commit -m 'feat: x'".into()),
        // an optional gate's failure does not gate the commit
        checks: vec![
            named("nextest", true, true, true),
            named("svelte-check", false, false, true),
        ],
        ..in_flight()
    };
    assert_eq!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn commit_allows_a_shipped_objective_with_its_green_checks() {
    let input = GuardInput {
        command: Some("git commit -m 'docs: roadmap'".into()),
        ..shipped()
    };
    assert!(matches!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Allow(Some(_))
    ));
}

// ── is_git_commit ───────────────────────────────────────────────────

#[test]
fn is_git_commit_recognises_the_usual_shapes() {
    assert!(is_git_commit("git commit -m x"));
    assert!(is_git_commit("git add -A && git commit -q -F -"));
    assert!(is_git_commit("GIT_AUTHOR_NAME=x git commit -m y"));
    assert!(is_git_commit(
        "/usr/bin/git -c commit.gpgsign=false commit -m y"
    ));
    assert!(is_git_commit("cargo fmt; git commit -am y"));
    assert!(!is_git_commit("git status --porcelain"));
    assert!(!is_git_commit("git log --oneline | cat"));
    assert!(!is_git_commit("echo 'git commit' > notes.txt"));
    assert!(!is_git_commit(""));
}

// ── Commit: latest result per gate, shipped objectives ──────────────

fn commit_input(status: &str, checks: Vec<CheckSummary>) -> GuardInput {
    GuardInput {
        objective_status: Some(status.into()),
        tasks_total: 3,
        checks,
        command: Some("git commit -m x".into()),
        ..GuardInput::default()
    }
}

#[test]
fn commit_allowed_when_a_failed_gate_was_re_run_green() {
    let input = commit_input(
        "active",
        vec![
            named("clippy", true, false, true),
            named("clippy", true, true, true),
        ],
    );
    assert_eq!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn commit_blocked_when_the_latest_run_of_a_gate_is_red() {
    let input = commit_input(
        "active",
        vec![
            named("clippy", true, true, true),
            named("clippy", true, false, true),
        ],
    );
    assert!(matches!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Block(reason) if reason.contains("FAILED")
    ));
}

#[test]
fn commit_blocked_when_another_gate_is_still_red() {
    let input = commit_input(
        "active",
        vec![
            named("clippy", true, false, true),
            named("nextest", true, true, true),
        ],
    );
    assert!(matches!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Block(_)
    ));
}

#[test]
fn commit_is_not_gated_once_the_objective_is_shipped() {
    let input = commit_input("completed", vec![named("clippy", true, false, true)]);
    assert!(matches!(
        evaluate(GuardEvent::Commit, &input),
        GuardVerdict::Allow(Some(note)) if note.contains("shipped")
    ));
}

// ── Write: frozen tree ──────────────────────────────────────────────

fn frozen() -> FrozenTree {
    FrozenTree {
        path: "crates/magistr-gui".into(),
        unlock: vec!["legacy-gui".into(), "delete".into()],
        replacement: Some("the native GUI (gui2-1..gui2-4)".into()),
    }
}

fn write_input(path: &str, status: Option<&str>, scope: Option<&str>) -> GuardInput {
    GuardInput {
        objective_status: status.map(Into::into),
        objective_scope: scope.map(Into::into),
        tasks_total: usize::from(status.is_some()),
        frozen: Some(frozen()),
        write_path: Some(path.into()),
        ..GuardInput::default()
    }
}

#[test]
fn write_is_not_gated_without_a_frozen_declaration() {
    let input = GuardInput {
        frozen: None,
        ..write_input(
            "crates/magistr-gui/src/App.svelte",
            Some("active"),
            Some("chunk:unrelated"),
        )
    };
    assert_eq!(
        evaluate(GuardEvent::Write, &input),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn write_is_not_gated_without_a_file_path() {
    let input = GuardInput {
        write_path: None,
        ..write_input("", Some("active"), Some("chunk:unrelated"))
    };
    assert_eq!(
        evaluate(GuardEvent::Write, &input),
        GuardVerdict::Allow(None)
    );
}

#[test]
fn write_allows_paths_outside_the_frozen_tree() {
    for path in [
        "crates/magistr-tui/src/lib.rs",
        // a sibling that merely shares the prefix
        "crates/magistr-gui-native/src/lib.rs",
        "src/main.rs",
        "tests/legacy_gui_guard.rs",
    ] {
        let input = write_input(path, Some("active"), Some("chunk:unrelated"));
        assert_eq!(
            evaluate(GuardEvent::Write, &input),
            GuardVerdict::Allow(None),
            "{path}"
        );
    }
}

#[test]
fn write_blocks_under_the_frozen_tree_with_an_unrelated_objective() {
    let input = write_input(
        "crates/magistr-gui/src/App.svelte",
        Some("active"),
        Some("chunk:acp-reconnect — src/acp/*.rs"),
    );
    match evaluate(GuardEvent::Write, &input) {
        GuardVerdict::Block(reason) => {
            assert!(
                reason.contains("crates/magistr-gui/src/App.svelte"),
                "{reason}"
            );
            assert!(reason.contains("frozen"), "{reason}");
            // the declaration's own words, not the code's
            assert!(
                reason.contains("the native GUI (gui2-1..gui2-4)"),
                "{reason}"
            );
            assert!(reason.contains("legacy-gui"), "{reason}");
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn write_blocks_under_the_frozen_tree_without_an_objective() {
    let input = write_input("crates/magistr-gui/src-tauri/src/main.rs", None, None);
    assert!(matches!(
        evaluate(GuardEvent::Write, &input),
        GuardVerdict::Block(_)
    ));
}

#[test]
fn write_allows_when_the_objective_scope_carries_an_unlock_word() {
    for scope in [
        "chunk:gui2-0-freeze-tauri — legacy-gui freeze",
        "chunk:gui2-4-cutover: delete crates/magistr-gui",
        "Legacy-GUI hotfix: the window will not open",
        "DELETE crates/magistr-gui",
    ] {
        let input = write_input(
            "crates/magistr-gui/src/App.svelte",
            Some("defined"),
            Some(scope),
        );
        assert!(
            matches!(
                evaluate(GuardEvent::Write, &input),
                GuardVerdict::Allow(Some(note)) if note.contains("frozen")
            ),
            "{scope}"
        );
    }
}

/// A scope only unlocks while the objective is in flight — a shipped one is
/// a record, not a licence.
#[test]
fn write_ignores_the_scope_of_a_shipped_objective() {
    let input = write_input(
        "crates/magistr-gui/src/App.svelte",
        Some("completed"),
        Some("chunk:gui2-0-freeze-tauri — legacy-gui freeze"),
    );
    assert!(matches!(
        evaluate(GuardEvent::Write, &input),
        GuardVerdict::Block(_)
    ));
}

#[test]
fn write_matches_the_frozen_root_itself() {
    let input = write_input(
        "crates/magistr-gui/README.md",
        Some("active"),
        Some("chunk:other"),
    );
    assert!(matches!(
        evaluate(GuardEvent::Write, &input),
        GuardVerdict::Block(_)
    ));
}

/// A declaration that names no replacement says nothing about where the work
/// belongs rather than guessing a tree — the message is the declaration's,
/// not the code's.
#[test]
fn write_block_omits_the_replacement_clause_when_none_is_declared() {
    let input = GuardInput {
        frozen: Some(FrozenTree {
            replacement: None,
            ..frozen()
        }),
        ..write_input(
            "crates/magistr-gui/src/App.svelte",
            Some("active"),
            Some("chunk:unrelated"),
        )
    };
    match evaluate(GuardEvent::Write, &input) {
        GuardVerdict::Block(reason) => {
            assert!(reason.contains("frozen"), "{reason}");
            assert!(!reason.contains("New work belongs"), "{reason}");
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

/// The unlock word is quoted from the declaration into the suggested scope,
/// so the message a second frozen tree prints is right without a code change.
#[test]
fn write_block_suggests_the_declared_unlock_word_as_the_hotfix_scope() {
    let input = GuardInput {
        frozen: Some(FrozenTree {
            path: "vendor/thing".into(),
            unlock: vec!["legacy-thing".into(), "delete".into()],
            replacement: Some("somewhere else".into()),
        }),
        ..write_input("vendor/thing/x.rs", Some("active"), Some("chunk:unrelated"))
    };
    match evaluate(GuardEvent::Write, &input) {
        GuardVerdict::Block(reason) => {
            assert!(
                reason.contains("ship_set_objective(scope: \"legacy-thing:"),
                "{reason}"
            );
            assert!(reason.contains("somewhere else"), "{reason}");
            assert!(!reason.contains("magistr-gui"), "{reason}");
        }
        other => panic!("expected Block, got {other:?}"),
    }
}
