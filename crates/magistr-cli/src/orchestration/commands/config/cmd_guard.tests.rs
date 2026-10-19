use super::*;
use magistr_cockpit::harness::fixtures;
use magistr_engine::{ChunkStatus, ShipData};
use serde_json::json;

fn ship(
    scope: &str,
    status: magistr_workflow::ship::domain::objective::ObjectiveStatus,
) -> ShipData {
    let mut objective = fixtures::objective("work");
    objective.scope = scope.into();
    objective.status = status;
    ShipData {
        current: Some(magistr_engine::ShipRun {
            run_id: "run".into(),
            task_instances: Default::default(),
            objective,
            tasks: Vec::new(),
        }),
        retained: Vec::new(),
    }
}

fn chunk(id: &str, status: ChunkStatus) -> magistr_workflow::roadmap::domain::Chunk {
    let mut chunk = fixtures::chunk(id);
    chunk.status = status;
    chunk
}

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
fn apply_engine_state_reads_validated_typed_records() {
    use magistr_workflow::ship::domain::objective::ObjectiveStatus;
    let mut ship = ship("chunk:b", ObjectiveStatus::Completed);
    ship.current.as_mut().unwrap().tasks = (0..4)
        .map(|id| fixtures::task(&id.to_string(), "work"))
        .collect();
    ship.current.as_mut().unwrap().tasks[0].checks = vec![
        fixtures::check("cargo check", true),
        fixtures::check("svelte-check", false),
        fixtures::check("manual", true),
    ];
    ship.current.as_mut().unwrap().tasks[0].checks[1].required = false;
    ship.current.as_mut().unwrap().tasks[0].checks[2].verified = false;
    let mut input = GuardInput::default();
    apply_engine_state(
        &mut input,
        &ship,
        &[
            chunk("a", ChunkStatus::Done),
            chunk("b", ChunkStatus::InProgress),
        ],
    );
    assert_eq!(input.objective_status.as_deref(), Some("completed"));
    assert_eq!(input.tasks_total, 4);
    assert_eq!(
        input
            .checks
            .iter()
            .map(|check| (
                check.name.as_str(),
                check.required,
                check.passed,
                check.verified
            ))
            .collect::<Vec<_>>(),
        [
            ("cargo check", true, true, true),
            ("svelte-check", false, false, true),
            ("manual", true, true, false)
        ]
    );
    assert!(input.chunk_in_progress);
}

#[test]
fn apply_engine_state_with_no_objective_and_no_chunks() {
    let mut input = GuardInput::default();
    apply_engine_state(&mut input, &ShipData::default(), &[]);
    assert_eq!(input.objective_status, None);
    assert_eq!(input.tasks_total, 0);
    assert!(input.checks.is_empty());
    assert!(!input.chunk_in_progress);
}

#[test]
fn stop_ignores_other_chunks_and_unscoped_objectives() {
    for scope in ["chunk:finished", "chunk:ongoing-extra", "fix:hooks", ""] {
        let mut input = GuardInput::default();
        apply_engine_state(
            &mut input,
            &ship(
                scope,
                magistr_workflow::ship::domain::objective::ObjectiveStatus::Completed,
            ),
            &[
                chunk("finished", ChunkStatus::Done),
                chunk("ongoing", ChunkStatus::InProgress),
            ],
        );
        assert_eq!(
            evaluate(GuardEvent::Stop, &input),
            GuardVerdict::Allow(None),
            "{scope}"
        );
    }
}

#[test]
fn stop_preserves_preexisting_and_untracked_files_after_completion() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let run = |args: &[&str]| {
        let result = std::process::Command::new("git")
            .args(args)
            .current_dir(tmp.path())
            .output()
            .expect("git");
        assert!(result.status.success(), "{result:?}");
        result.stdout
    };
    run(&["init", "-q"]);
    std::fs::write(tmp.path().join("workflow.md"), "old").expect("write");
    run(&["add", "workflow.md"]);
    run(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-qm",
        "baseline",
    ]);
    std::fs::write(tmp.path().join("workflow.md"), "other session").expect("write");
    std::fs::write(tmp.path().join("notes.txt"), "untracked").expect("write");
    let before = run(&["status", "--porcelain"]);
    assert!(!before.is_empty());
    let mut input = GuardInput::default();
    apply_engine_state(
        &mut input,
        &ship(
            "chunk:work",
            magistr_workflow::ship::domain::objective::ObjectiveStatus::Completed,
        ),
        &[chunk("work", ChunkStatus::Done)],
    );
    assert_eq!(
        evaluate(GuardEvent::Stop, &input),
        GuardVerdict::Allow(None)
    );
    assert_eq!(run(&["status", "--porcelain"]), before);
    assert!(!tmp.path().join("ROADMAP.md").exists());
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
fn workbench_declared_reads_the_mcp_declaration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert!(!workbench_declared(tmp.path()), "no file, no workbench");

    let file = tmp.path().join(MCP_FILE);
    std::fs::write(
        &file,
        r#"{"mcpServers": {"magistr": {"command": "magistr"}}}"#,
    )
    .expect("write");
    assert!(
        !workbench_declared(tmp.path()),
        "magistr alone is not workbench"
    );

    std::fs::write(
        &file,
        r#"{"mcpServers": {"workbench-mcp": {"command": "magistr", "args": ["mcp", "--dir", "."]}}}"#,
    )
    .expect("write");
    assert!(workbench_declared(tmp.path()));

    std::fs::write(&file, "not json").expect("write");
    assert!(
        !workbench_declared(tmp.path()),
        "unreadable declaration routes nothing"
    );
}

#[test]
fn apply_engine_state_reads_the_objective_scope() {
    let ship = ship(
        "chunk:gui2-0-freeze-tauri — legacy-gui freeze",
        magistr_workflow::ship::domain::objective::ObjectiveStatus::Active,
    );
    let mut input = GuardInput::default();
    apply_engine_state(&mut input, &ship, &[]);
    assert_eq!(
        input.objective_scope.as_deref(),
        Some("chunk:gui2-0-freeze-tauri — legacy-gui freeze")
    );
}

#[test]
fn unreadable_required_owner_refuses_guard_state_and_repair_recovers() {
    use magistr_engine::{NativeConfig, NativeReader};
    use magistr_workflow::infra::{Domain, Persistence};
    let temp = tempfile::tempdir().unwrap();
    let config = NativeConfig::from_lookup(temp.path().to_owned(), |_| None)
        .with_data_dir(temp.path().join("data"));
    let reader = NativeReader::open_for_project(&config, "guard-read".into());
    let mut input = GuardInput::default();
    read_engine_state(&mut input, &reader).unwrap();
    let path =
        Persistence::new(&config.persistence_config(), Domain::Roadmap).path_for("guard-read");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "broken records").unwrap();
    assert!(read_engine_state(&mut input, &reader).is_err());
    assert!(
        read_engine_state(&mut input, &reader).is_err(),
        "unchanged bad file still refuses"
    );
    std::fs::remove_file(path).unwrap();
    read_engine_state(&mut input, &reader).unwrap();
    assert_eq!(input, GuardInput::default());
}
