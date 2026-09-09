use super::*;
use serde_json::json;

#[test]
fn hook_fields_read_stop_hook_active_and_the_bash_command() {
    let stop = hook_fields(&json!({"hook_event_name": "Stop", "stop_hook_active": true}));
    assert!(stop.stop_hook_active);
    assert_eq!(stop.command, None);

    let pre = hook_fields(&json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "git commit -m x", "description": "commit"}
    }));
    assert!(!pre.stop_hook_active);
    assert_eq!(pre.command.as_deref(), Some("git commit -m x"));
}

#[test]
fn hook_fields_are_inert_on_null_or_foreign_json() {
    let input = hook_fields(&serde_json::Value::Null);
    assert_eq!(input, GuardInput::default());
    let input = hook_fields(&json!({"something": "else"}));
    assert_eq!(input, GuardInput::default());
}

#[test]
fn apply_engine_state_reads_the_ship_and_roadmap_documents() {
    let ship = json!({
        "objective": {"status": "completed", "description": "x"},
        "tasks": {"total": 4, "completed": 4},
        "checks": [
            {"name": "cargo check", "passed": true, "required": true, "verified": true},
            {"name": "svelte-check", "passed": false, "required": false, "verified": true},
            {"name": "manual", "passed": true, "required": true}
        ]
    });
    let roadmap = json!({"chunks": [
        {"id": "a", "status": "done"},
        {"id": "b", "status": "in_progress"}
    ]});
    let mut input = GuardInput::default();
    apply_engine_state(&mut input, &ship, &roadmap);
    assert_eq!(input.objective_status.as_deref(), Some("completed"));
    assert_eq!(input.tasks_total, 4);
    assert_eq!(
        input.checks,
        vec![
            CheckSummary {
                name: "cargo check".into(),
                required: true,
                passed: true,
                verified: true
            },
            CheckSummary {
                name: "svelte-check".into(),
                required: false,
                passed: false,
                verified: true
            },
            CheckSummary {
                name: "manual".into(),
                required: true,
                passed: true,
                verified: false
            },
        ]
    );
    assert!(input.chunk_in_progress);
}

#[test]
fn apply_engine_state_with_no_objective_and_no_chunks() {
    let mut input = GuardInput::default();
    apply_engine_state(
        &mut input,
        &json!({"objective": null, "tasks": {"total": 0}, "checks": []}),
        &json!({"chunks": []}),
    );
    assert_eq!(input.objective_status, None);
    assert_eq!(input.tasks_total, 0);
    assert!(input.checks.is_empty());
    assert!(!input.chunk_in_progress);
}

#[test]
fn git_dirty_is_false_outside_a_repository() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Not a git repo → `git status` fails → treated as clean.
    assert!(!git_dirty(tmp.path()));
}

#[test]
fn git_dirty_tracks_the_working_tree() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(tmp.path())
            .output()
            .expect("git")
    };
    run(&["init", "-q"]);
    assert!(!git_dirty(tmp.path()), "fresh repo is clean");
    std::fs::write(tmp.path().join("a.txt"), "x").expect("write");
    assert!(git_dirty(tmp.path()), "untracked file is dirty");
}

// ── Write: hook fields, path resolution, frozen declaration ─────────

#[test]
fn hook_fields_read_the_file_path_of_an_edit_and_the_notebook_path() {
    let edit = hook_fields(&json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Edit",
        "tool_input": {"file_path": "/proj/crates/magistr-gui/src/App.svelte", "old_string": "a", "new_string": "b"}
    }));
    assert_eq!(
        edit.write_path.as_deref(),
        Some("/proj/crates/magistr-gui/src/App.svelte")
    );
    assert_eq!(edit.command, None);

    let nb = hook_fields(&json!({
        "tool_name": "NotebookEdit",
        "tool_input": {"notebook_path": "/proj/notes.ipynb"}
    }));
    assert_eq!(nb.write_path.as_deref(), Some("/proj/notes.ipynb"));
}

#[test]
fn relative_write_path_resolves_absolute_relative_and_dotted_paths() {
    let root = Path::new("/proj");
    let roots = [root];
    const REL: &str = "crates/magistr-gui/src/App.svelte";
    assert_eq!(
        relative_write_path(&roots, "/proj/crates/magistr-gui/src/App.svelte").as_deref(),
        Some(REL)
    );
    assert_eq!(relative_write_path(&roots, REL).as_deref(), Some(REL));
    assert_eq!(
        relative_write_path(&roots, "/proj/crates/./magistr-gui/x/../src/App.svelte").as_deref(),
        Some(REL)
    );
    assert_eq!(relative_write_path(&roots, "/elsewhere/x.rs"), None);
    assert_eq!(relative_write_path(&roots, "/proj/../other/x.rs"), None);
}

#[cfg(unix)]
#[test]
fn relative_write_path_sees_through_a_symlinked_project_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let real = tmp.path().join("real");
    std::fs::create_dir_all(real.join("crates/magistr-gui/src")).expect("mkdir");
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let canonical = link.canonicalize().expect("canonicalize");
    // The hook names the file through the symlink; the guard canonicalized the dir.
    let raw = link
        .join("crates/magistr-gui/src/New.svelte")
        .display()
        .to_string();
    assert_eq!(
        relative_write_path(&[&canonical, &link], &raw).as_deref(),
        Some("crates/magistr-gui/src/New.svelte")
    );
}

#[test]
fn read_frozen_parses_the_declaration_and_defaults_the_unlock_words() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert_eq!(read_frozen(tmp.path()), None, "no file, nothing frozen");

    std::fs::create_dir_all(tmp.path().join(".magistr")).expect("mkdir");
    let file = tmp.path().join(RATCHET_FILE);
    std::fs::write(
        &file,
        r#"{"path": "crates/magistr-gui/", "files": 3, "loc": 10,
            "unlock": ["hotfix"], "replacement": "the native GUI"}"#,
    )
    .expect("write");
    assert_eq!(
        read_frozen(tmp.path()),
        Some(FrozenTree {
            path: "crates/magistr-gui".into(),
            unlock: vec!["hotfix".into()],
            replacement: Some("the native GUI".into()),
        })
    );

    // No unlock list falls back to `delete` alone — the one verb every
    // freeze admits — and no replacement stays None rather than guessing.
    std::fs::write(
        &file,
        r#"{"path": "crates/magistr-gui", "files": 3, "loc": 10, "replacement": "  "}"#,
    )
    .expect("write");
    let frozen = read_frozen(tmp.path()).expect("declared");
    assert_eq!(frozen.unlock, vec!["delete".to_owned()]);
    assert_eq!(
        frozen.replacement, None,
        "a blank replacement is no replacement"
    );

    std::fs::write(&file, "not json").expect("write");
    assert_eq!(
        read_frozen(tmp.path()),
        None,
        "unreadable declaration never blocks"
    );
}

/// The declaration this repo actually ships freezes the Tauri GUI.
#[test]
fn this_repo_declares_the_tauri_gui_frozen() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let frozen = read_frozen(root).expect("the project declares a frozen tree");
    assert_eq!(frozen.path, "crates/magistr-gui");
    assert!(
        frozen.unlock.iter().any(|w| w == "legacy-gui"),
        "{frozen:?}"
    );
    assert!(frozen.unlock.iter().any(|w| w == "delete"), "{frozen:?}");
    assert!(frozen.replacement.is_some(), "{frozen:?}");
}

#[test]
fn apply_engine_state_reads_the_objective_scope() {
    let ship = json!({
        "objective": {"status": "active", "scope": "chunk:gui2-0-freeze-tauri — legacy-gui freeze"},
        "tasks": {"total": 1},
        "checks": []
    });
    let mut input = GuardInput::default();
    apply_engine_state(&mut input, &ship, &json!({"chunks": []}));
    assert_eq!(
        input.objective_scope.as_deref(),
        Some("chunk:gui2-0-freeze-tauri — legacy-gui freeze")
    );
}
