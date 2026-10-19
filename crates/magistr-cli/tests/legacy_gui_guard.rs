//! `magistr guard write` end to end: the frozen tree declared in
//! `.magistr/legacy-gui-ratchet.json` is only writable under a ship objective
//! whose scope carries an unlock word. Drives the real binary and the
//! scaffolded hook script against a temp project on a temp magistr
//! store.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use magistr_engine::{NativeConfig, NativeEngines, ProjectAuthority, WriterRole};
use tempfile::TempDir;

const RATCHET: &str = r#"{"path": "crates/magistr-gui", "files": 1, "loc": 1,
    "unlock": ["legacy-gui", "delete"],
    "replacement": "the native GUI (roadmap gui2-1..gui2-4)"}"#;

struct Project {
    _tmp: TempDir,
    root: PathBuf,
    data_dir: PathBuf,
}

impl Project {
    fn new(frozen: bool) -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().join("proj");
        let data_dir = tmp.path().join("tas-data");
        std::fs::create_dir_all(root.join(".magistr")).expect("mkdir");
        std::fs::create_dir_all(root.join("crates/magistr-gui/src")).expect("mkdir");
        std::fs::write(root.join("crates/magistr-gui/src/App.svelte"), "<div />\n").expect("write");
        std::fs::write(
            root.join(".magistr/project.json"),
            r#"{"project": {"name": "t", "description": "", "type": "cli"},
                "quality_gate": {"check": "true", "test": "true", "lint": "true"},
                "roadmap_backend": "native"}"#,
        )
        .expect("write");
        if frozen {
            std::fs::write(root.join(".magistr/legacy-gui-ratchet.json"), RATCHET).expect("write");
        }
        Self {
            _tmp: tmp,
            root,
            data_dir,
        }
    }

    /// A host writer on the fixture store; dropped after each mutation so
    /// the guard subprocess (a reader) and the next mutation see it free.
    fn engines(&self) -> NativeEngines {
        let data_dir = self.data_dir.display().to_string();
        let config = NativeConfig::from_lookup(&self.root, |key| {
            (key == "MAGISTR_WORKFLOW_DATA_DIR").then(|| data_dir.clone())
        });
        ProjectAuthority::admit(&config, WriterRole::Host)
            .expect("host admitted")
            .open()
    }

    fn set_objective(&self, scope: &str) {
        self.engines()
            .ship_set_objective("objective".into(), vec![], vec![], scope.into())
            .expect("objective");
    }

    fn hook_json(&self, rel: &str) -> String {
        let file = self.root.join(rel);
        serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Edit",
            "tool_input": {"file_path": file, "old_string": "a", "new_string": "b"}
        })
        .to_string()
    }

    /// `magistr guard write <root>` with the hook JSON on stdin.
    fn guard(&self, rel: &str) -> Output {
        self.run(
            magistr_command::magistr_command()
                .args(["guard", "write"])
                .arg(&self.root),
            rel,
        )
    }

    /// The scaffolded hook script, pointed at the built binary.
    #[cfg(unix)]
    fn hook_script(&self, rel: &str) -> Output {
        let script = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join(".claude/hooks/magistr-guard.sh");
        self.run(
            Command::new(script)
                .arg("write")
                .env("MAGISTR_BIN", env!("CARGO_BIN_EXE_magistr"))
                .env("CLAUDE_PROJECT_DIR", &self.root),
            rel,
        )
    }

    fn run(&self, cmd: &mut Command, rel: &str) -> Output {
        use std::io::Write as _;
        let mut child = cmd
            .env("MAGISTR_WORKFLOW_DATA_DIR", &self.data_dir)
            .env_remove("MAGISTR_WORKFLOW_BROADCAST")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(self.hook_json(rel).as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("wait")
    }
}

#[test]
fn stop_hook_allows_completed_work_with_unrelated_changes_and_exports_disabled() {
    let project = Project::new(false);
    let config_path = project.root.join(".magistr/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
    config["roadmap_exports"] = serde_json::json!(false);
    std::fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "baseline",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&project.root)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let changed = project.root.join("crates/magistr-gui/src/App.svelte");
    std::fs::write(&changed, "other session's work").unwrap();
    std::fs::write(project.root.join("notes.txt"), "untracked notes").unwrap();
    let status = || {
        Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&project.root)
            .output()
            .unwrap()
            .stdout
    };
    let before = status();
    assert!(!before.is_empty());
    {
        let engines = project.engines();
        engines
            .ship_set_objective(
                "finished work".into(),
                vec![],
                vec![],
                "fix:finished".into(),
            )
            .unwrap();
        engines.ship_finalize(vec![], None).unwrap();
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/hooks/magistr-guard.sh");
    let out = project.run(
        Command::new("sh")
            .arg(script)
            .arg("stop")
            .env("MAGISTR_BIN", env!("CARGO_BIN_EXE_magistr"))
            .env("CLAUDE_PROJECT_DIR", &project.root),
        "",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!stderr(&out).contains("stage everything"));
    assert_eq!(status(), before);
    assert_eq!(
        std::fs::read_to_string(changed).unwrap(),
        "other session's work"
    );
    assert!(!project.root.join("ROADMAP.md").exists());
    assert!(!project.root.join("ROADMAP.json").exists());
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_write_under_the_frozen_tree_is_denied_under_an_unrelated_objective() {
    let project = Project::new(true);
    project.set_objective("chunk:acp-reconnect — src/acp/*.rs");
    let out = project.guard("crates/magistr-gui/src/App.svelte");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("frozen"), "{err}");
    // The declaration's own words reach the agent.
    assert!(err.contains("the native GUI"), "{err}");
    assert!(err.contains("legacy-gui"), "{err}");
}

#[test]
fn a_write_under_the_frozen_tree_is_denied_without_an_objective() {
    let project = Project::new(true);
    let out = project.guard("crates/magistr-gui/src/NewPanel.svelte");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
}

#[test]
fn a_legacy_gui_scoped_objective_unlocks_the_tree() {
    let project = Project::new(true);
    project.set_objective("chunk:gui2-0-freeze-tauri — legacy-gui freeze");
    let out = project.guard("crates/magistr-gui/src/App.svelte");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).contains("admitted"), "{}", stderr(&out));
}

#[test]
fn a_delete_scoped_objective_unlocks_the_tree() {
    let project = Project::new(true);
    project.set_objective("chunk:gui2-4-cutover: delete crates/magistr-gui");
    let out = project.guard("crates/magistr-gui/src-tauri/src/main.rs");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn writes_outside_the_frozen_tree_pass() {
    let project = Project::new(true);
    project.set_objective("chunk:unrelated");
    for rel in [
        "crates/magistr-desktop/src/lib.rs",
        // a sibling that merely shares the prefix
        "crates/magistr-gui-native/src/lib.rs",
        "src/main.rs",
    ] {
        let out = project.guard(rel);
        assert_eq!(out.status.code(), Some(0), "{rel}: {}", stderr(&out));
    }
}

#[test]
fn a_project_without_a_ratchet_declaration_is_never_gated() {
    let project = Project::new(false);
    project.set_objective("chunk:unrelated");
    let out = project.guard("crates/magistr-gui/src/App.svelte");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[cfg(unix)]
#[test]
fn the_hook_script_forwards_the_write_event_and_the_block() {
    let project = Project::new(true);
    project.set_objective("chunk:unrelated");
    let out = project.hook_script("crates/magistr-gui/src/App.svelte");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("frozen"), "{}", stderr(&out));

    project.set_objective("legacy-gui hotfix: the window will not open");
    let out = project.hook_script("crates/magistr-gui/src/App.svelte");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// The mechanism is not the GUI's: a declaration naming any other tree, with
/// its own unlock word, blocks and admits exactly the same way — which is
/// what makes the next freeze a one-file change.
#[test]
fn the_freeze_is_declared_not_hardcoded() {
    let project = Project::new(true);
    std::fs::write(
        project.root.join(".magistr/legacy-gui-ratchet.json"),
        r#"{"path": "vendor/thing", "files": 1, "loc": 1, "unlock": ["legacy-thing"]}"#,
    )
    .expect("write");
    project.set_objective("chunk:unrelated");

    // The GUI is no longer the frozen tree.
    let out = project.guard("crates/magistr-gui/src/App.svelte");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let out = project.guard("vendor/thing/x.rs");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("legacy-thing"), "{}", stderr(&out));

    project.set_objective("legacy-thing hotfix");
    let out = project.guard("vendor/thing/x.rs");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// The gate the hook depends on is wired: `.claude/settings.json` routes the
/// file-editing tools at `magistr-guard.sh write`. Without the matcher the
/// binary above is never asked.
#[test]
fn this_repo_wires_the_write_hook() {
    let settings = std::fs::read_to_string(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(".claude/settings.json"),
    )
    .expect("read settings.json");
    let json: serde_json::Value = serde_json::from_str(&settings).expect("settings.json is JSON");
    let matchers = json["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse hooks");
    let write = matchers
        .iter()
        .find(|entry| {
            entry["matcher"]
                .as_str()
                .is_some_and(|m| m.contains("Edit"))
        })
        .expect("a matcher on the file-editing tools");
    assert_eq!(
        write["matcher"].as_str(),
        Some("Edit|Write|MultiEdit|NotebookEdit")
    );
    assert!(
        write["hooks"][0]["command"]
            .as_str()
            .is_some_and(|c| c.ends_with("magistr-guard.sh write")),
        "{write}"
    );
}
