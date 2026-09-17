//! The operational workspace verbs as the real binary (chunk arch-14c10b):
//! `magistr workspace doctor [bundle|config]`, `journal`, `key rotate`,
//! `init` and `snapshot` with the retained renderings and process exit codes — 0 healthy,
//! 1 findings, 64 usage, 65 durable data this binary cannot read, 66
//! missing root — read-only and lock-free beside a live owner.
#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Output,
};

use magistr_command::magistr_command;
use magistr_workspace::{
    file_content::FileContent,
    journal::{JournalDb, JournalDbOptions, JournalFileChange, JournalFileKind, JournalOpInit},
    service::ServerLock,
};
use serde_json::Value;

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::write(dir.path().join("a.ts"), "export const a = 1;\n").expect("source");
    dir
}

/// A workspace whose journal holds two ops, the second an edit of the first.
fn seeded() -> tempfile::TempDir {
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let mut journal =
        JournalDb::open(JournalDbOptions::new(state.join("journal.db"))).expect("journal");
    let change = |kind, before: &str, after: &str| JournalFileChange {
        kind,
        path: "src/a.ts".into(),
        before: FileContent::Text(before.into()),
        after: FileContent::Text(after.into()),
    };
    journal
        .append(&JournalOpInit {
            tool: "edit".into(),
            args_hash: "h1".into(),
            session_id: "alice".into(),
            files: vec![change(JournalFileKind::Create, "", "const a = 1\n")],
            undo_of: None,
        })
        .expect("first op");
    journal
        .append(&JournalOpInit {
            tool: "files".into(),
            args_hash: "h2".into(),
            session_id: "bob".into(),
            files: vec![change(
                JournalFileKind::Write,
                "const a = 1\n",
                "const a = 2\n",
            )],
            undo_of: None,
        })
        .expect("second op");
    dir
}

/// Seeded findings: a quarantined input and a breakered plugin whose
/// inventory names it.
fn seed_findings(root: &Path) {
    let state = root.join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    std::fs::write(
        state.join("quarantine.jsonl"),
        "{\"path\":\"weird.bin\",\"strikes\":3,\"quarantined\":true,\"lastCrash\":\"SIGSEGV\",\"at\":1755000000000}\nthis line is torn and not json\n",
    )
    .expect("quarantine");
}

fn run(args: &[&str], cwd: &Path) -> Output {
    let mut command = magistr_command();
    command.arg("workspace").args(args).current_dir(cwd);
    command.env("WORKBENCH_NETWORK_FS", "0");
    command.output().expect("run the binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn code(output: &Output) -> Option<i32> {
    output.status.code()
}

/// Hashes of everything under the state dir except the WAL shared-memory
/// index: SQLite mutates that sidecar for any reader, read-only opens
/// included, and it holds no durable state.
fn snapshot(state: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(state)
        .expect("state dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| !entry.file_name().to_string_lossy().ends_with("-shm"))
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (name, std::fs::read(entry.path()).expect("read"))
        })
        .collect()
}

fn envelope(output: &Output) -> Value {
    serde_json::from_str(stdout(output).trim()).expect("one JSON envelope on stdout")
}

fn seqs(envelope: &Value) -> Vec<i64> {
    envelope["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|row| row["seq"].as_i64().expect("seq"))
        .collect()
}

#[test]
fn journal_prints_the_log_as_one_envelope_of_rows_newest_op_first() {
    let dir = seeded();
    let root = dir.path().to_str().expect("utf8 root");
    let listed = run(&["journal", root, "--json"], dir.path());
    assert_eq!(code(&listed), Some(0), "{}", stderr(&listed));
    let envelope = envelope(&listed);
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["budget"]["requested"], 0);
    assert_eq!(seqs(&envelope), [2, 1]);
    let newest = &envelope["results"][0];
    assert_eq!(newest["tool"], "files");
    assert_eq!(newest["sessionId"], "bob");
    assert_eq!(newest["path"], "src/a.ts");
    assert_eq!(newest["added"], 1);
    assert_eq!(newest["removed"], 1);
    assert_eq!(newest["undoable"], true);
    assert!(
        newest["diff"]
            .as_str()
            .is_some_and(|diff| diff.contains("+const a = 2"))
    );
}

#[test]
fn journal_and_doctor_run_beside_a_live_owner_without_taking_or_disturbing_the_lock() {
    let dir = seeded();
    let root = dir.path().to_str().expect("utf8 root");
    let state = dir.path().join(".workbench");
    seed_findings(dir.path());
    // A live instance owns the workspace, exactly as during `magistr run`:
    // the OS lock is held and the journal writer stays open.
    let lock = ServerLock::acquire(state.join(ServerLock::FILE_NAME)).expect("live owner");
    let writer = JournalDb::open(JournalDbOptions::new(state.join("journal.db"))).expect("writer");
    let before = snapshot(&state);
    let listed = run(&["journal", root, "--json"], dir.path());
    assert_eq!(code(&listed), Some(0), "{}", stderr(&listed));
    assert_eq!(seqs(&envelope(&listed)), [2, 1]);
    let report = run(&["doctor", root], dir.path());
    assert_eq!(code(&report), Some(1), "{}", stderr(&report));
    assert!(stdout(&report).contains("[quarantine]"));
    assert!(stdout(&report).contains("weird.bin quarantined after 3 strike(s)"));
    let target = dir.path().join("bundle.tar.gz");
    let bundle = run(
        &[
            "doctor",
            "bundle",
            root,
            "--out",
            target.to_str().expect("utf8"),
        ],
        dir.path(),
    );
    assert_eq!(code(&bundle), Some(1), "{}", stderr(&bundle));
    assert!(target.is_file());
    assert!(stderr(&bundle).contains("workbench-doctor/quarantine.json"));
    // A diagnostic never destroys a prior diagnostic.
    let again = run(
        &[
            "doctor",
            "bundle",
            root,
            "--out",
            target.to_str().expect("utf8"),
        ],
        dir.path(),
    );
    assert_eq!(code(&again), Some(64));
    assert!(stderr(&again).contains("bundle target already exists"));
    assert_eq!(snapshot(&state), before, "the state dir was rewritten");
    assert!(state.join(ServerLock::FILE_NAME).is_file());
    drop(writer);
    drop(lock);
}

#[test]
fn journal_flags_narrow_the_listing_and_the_text_rendering_names_the_op() {
    let dir = seeded();
    let root = dir.path().to_str().expect("utf8 root");
    let rows = |flags: &[&str]| {
        let mut args = vec!["journal", root, "--json"];
        args.extend_from_slice(flags);
        let output = run(&args, dir.path());
        assert_eq!(code(&output), Some(0), "{}", stderr(&output));
        envelope(&output)
    };
    assert_eq!(seqs(&rows(&["--limit", "1"])), [2]);
    assert_eq!(seqs(&rows(&["--before", "2"])), [1]);
    assert_eq!(seqs(&rows(&["--since", "2"])), [2]);
    assert_eq!(seqs(&rows(&["--session", "alice"])), [1]);
    let lean = rows(&["--no-diff"]);
    assert!(lean["results"][0].get("diff").is_none());
    assert_eq!(lean["results"][0]["added"], 1);

    let text = run(&["journal", root, "--no-diff"], dir.path());
    assert_eq!(code(&text), Some(0));
    let out = stdout(&text);
    assert!(out.contains("#2"), "{out}");
    assert!(out.contains("  files  session=bob  undoable"), "{out}");
    assert!(out.contains("M src/a.ts  +1 -1"), "{out}");
    assert!(out.contains("A src/a.ts  +1 -0"), "{out}");
}

#[test]
fn a_workspace_that_never_wrote_says_so_and_misuse_is_a_usage_error() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let fresh = run(&["journal", root, "--json"], dir.path());
    assert_eq!(code(&fresh), Some(0));
    let envelope = envelope(&fresh);
    assert_eq!(envelope["results"], Value::Array(Vec::new()));
    assert!(envelope["notes"][0].as_str().is_some_and(|note| {
        note.starts_with("no journal yet at ") && note.ends_with("through this workspace")
    }));
    let text = run(&["journal", root], dir.path());
    assert_eq!(code(&text), Some(0));
    assert!(stdout(&text).is_empty());
    assert!(stderr(&text).contains("no journal yet at"));
    assert!(
        !dir.path().join(".workbench").exists(),
        "journal created state"
    );

    assert_eq!(
        code(&run(&["journal", root, "--nope"], dir.path())),
        Some(64)
    );
    assert_eq!(
        code(&run(&["journal", root, "--limit", "0"], dir.path())),
        Some(64)
    );
    assert_eq!(
        code(&run(&["journal", root, "--limit", "201"], dir.path())),
        Some(64)
    );
    assert_eq!(
        code(&run(&["journal", root, "--since", "-1"], dir.path())),
        Some(64)
    );
    let missing = run(&["journal", "missing", "--json"], dir.path());
    assert_eq!(code(&missing), Some(66));
    assert!(stderr(&missing).contains("workspace root does not exist"));
    // `--out` belongs to `doctor bundle` alone; a malformed `--config` is usage.
    assert_eq!(
        code(&run(&["doctor", root, "--out", "x.tar.gz"], dir.path())),
        Some(64)
    );
    let garbage = run(
        &["doctor", "config", root, "--config", "garbage"],
        dir.path(),
    );
    assert_eq!(code(&garbage), Some(64));
    assert!(stderr(&garbage).contains("--config expects key=value"));
}

#[test]
fn a_journal_written_by_a_newer_binary_fails_closed_with_the_file_untouched() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let path = state.join("journal.db");
    let raw = rusqlite::Connection::open(&path).expect("raw journal");
    raw.execute_batch("PRAGMA user_version = 999")
        .expect("stamp");
    drop(raw);
    let before = std::fs::read(&path).expect("bytes");
    let listed = run(&["journal", root, "--json"], dir.path());
    assert_eq!(code(&listed), Some(65), "{}", stderr(&listed));
    assert!(stderr(&listed).contains("cannot be read"));
    assert_eq!(std::fs::read(&path).expect("bytes"), before);
}

#[test]
fn a_healthy_workspace_exits_zero_and_doctor_config_renders_provenance() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let healthy = run(&["doctor", root], dir.path());
    assert_eq!(code(&healthy), Some(0), "{}", stderr(&healthy));
    let out = stdout(&healthy);
    assert!(out.starts_with("workbench doctor — workspace "), "{out}");
    for section in [
        "[environment]",
        "[index]",
        "[config]",
        "[plugins]",
        "[quarantine]",
        "[crashes]",
        "[owner]",
        "[operations]",
    ] {
        assert!(out.contains(section), "{out}");
    }
    assert!(out.contains("no owner reachable"), "{out}");
    assert!(out.contains("no run checkpoint"), "{out}");
    assert!(
        out.contains("no native index yet (fresh workspace)"),
        "{out}"
    );
    assert!(
        out.ends_with("result: all probes healthy (exit 0)\n"),
        "{out}"
    );
    // The probes never create the state dir (read-only).
    assert!(!dir.path().join(".workbench").exists());

    let config = run(
        &["doctor", "config", root, "--config", "budgets.default=9999"],
        dir.path(),
    );
    assert_eq!(code(&config), Some(0), "{}", stderr(&config));
    let out = stdout(&config);
    assert!(
        out.contains("workbench doctor config — workspace "),
        "{out}"
    );
    assert!(
        out.contains("effective config (value ← deciding layer):"),
        "{out}"
    );
    assert!(out.contains("budgets.default = 9999  (session)"), "{out}");
    assert!(!dir.path().join(".workbench").exists());
}

#[test]
fn a_bundle_lands_under_the_retained_default_name_in_the_current_directory() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let cwd = tempfile::tempdir().expect("cwd");
    let bundle = run(&["doctor", "bundle", root], cwd.path());
    assert_eq!(code(&bundle), Some(0), "{}", stderr(&bundle));
    let written: Vec<PathBuf> = std::fs::read_dir(cwd.path())
        .expect("cwd")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(written.len(), 1, "{written:?}");
    let name = written[0].file_name().expect("name").to_string_lossy();
    assert!(name.starts_with("workbench-doctor-bundle-") && name.ends_with(".tar.gz"));
    assert!(stderr(&bundle).contains("workbench-doctor/manifest.json"));
}

#[test]
fn key_rotate_needs_a_served_workspace_and_then_replaces_the_key() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let refused = run(&["key", "rotate", root], dir.path());
    assert_eq!(code(&refused), Some(66));
    assert!(stderr(&refused).contains("not a workbench workspace"));
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let first = run(&["key", "rotate", root], dir.path());
    assert_eq!(code(&first), Some(0), "{}", stderr(&first));
    assert!(stderr(&first).contains("workspace key rotated; outstanding confirm tokens are void"));
    let key_path = state.join("keys/workspace.key");
    let before = std::fs::read(&key_path).expect("key");
    assert_eq!(before.len(), 32);
    let second = run(&["key", "rotate", root], dir.path());
    assert_eq!(code(&second), Some(0));
    let after = std::fs::read(&key_path).expect("key");
    assert_eq!(after.len(), 32);
    assert_ne!(after, before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&key_path)
            .expect("mode")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn init_seeds_the_commands_table_once_and_never_touches_an_existing_one() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let nothing = run(&["init", root], dir.path());
    assert_eq!(code(&nothing), Some(0));
    assert!(stderr(&nothing).contains("no project commands detected — nothing seeded"));
    assert!(!dir.path().join("workbench.toml").exists());

    std::fs::write(
        dir.path().join("package.json"),
        "{\"scripts\":{\"test\":\"bun test\"}}",
    )
    .expect("manifest");
    std::fs::write(dir.path().join("bun.lock"), "").expect("lockfile");
    std::fs::write(
        dir.path().join("workbench.toml"),
        "[edit]\nconfirm_mode = \"token\"\n",
    )
    .expect("existing config");
    let seeded = run(&["init", root], dir.path());
    assert_eq!(code(&seeded), Some(0), "{}", stderr(&seeded));
    assert!(stderr(&seeded).contains("seeded 1 command(s) into"));
    assert!(
        stderr(&seeded)
            .contains("  test = \"bun run test\"  (package.json scripts.test + bun.lock)")
    );
    let toml = std::fs::read_to_string(dir.path().join("workbench.toml")).expect("toml");
    assert!(
        toml.starts_with("[edit]\nconfirm_mode = \"token\"\n\n# Seeded by"),
        "{toml}"
    );
    assert!(toml.contains(
        "[commands]\n\"test\" = \"bun run test\" # package.json scripts.test + bun.lock\n"
    ));

    let untouched = run(&["init", root], dir.path());
    assert_eq!(code(&untouched), Some(0));
    assert!(stderr(&untouched).contains("already declares [commands] — left untouched"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("workbench.toml")).expect("toml"),
        toml
    );
    assert_eq!(code(&run(&["init", "missing"], dir.path())), Some(66));
}

#[test]
fn snapshot_prints_the_source_observation_beside_a_live_owner_and_refuses_bad_inputs() {
    let dir = seeded();
    let root = dir.path().to_str().expect("utf8 root");
    let state = dir.path().join(".workbench");
    std::fs::write(dir.path().join(".gitignore"), "target/\n").expect("ignore");
    std::fs::create_dir_all(dir.path().join("target")).expect("target");
    std::fs::write(dir.path().join("target/built"), "artifact").expect("artifact");
    let lock = ServerLock::acquire(state.join(ServerLock::FILE_NAME)).expect("live owner");
    let generation = lock.generation().get();
    let before = snapshot(&state);
    let printed = run(
        &["snapshot", root, "--artifact", "target/built"],
        dir.path(),
    );
    assert_eq!(code(&printed), Some(0), "{}", stderr(&printed));
    let parsed: Value = serde_json::from_str(stdout(&printed).trim()).expect("one snapshot");
    assert_eq!(parsed["scheme"], "magistr-source-snapshot/1");
    assert_eq!(parsed["consistency"], "optimistic");
    assert_eq!(parsed["identity"]["generation"], Value::from(generation));
    assert_eq!(
        parsed["identity"]["root"].as_str().map(PathBuf::from),
        Some(dir.path().canonicalize().expect("canonical"))
    );
    let entries = parsed["manifest"]["entries"].as_object().expect("entries");
    assert_eq!(entries["a.ts"]["kind"], "text");
    assert_eq!(entries["target/built"]["kind"], "text");
    assert_eq!(entries["workbench.toml"]["kind"], "missing");
    assert!(!entries.contains_key(".workbench/journal.db"));
    assert_eq!(snapshot(&state), before, "the state dir was rewritten");
    let refused = run(&["snapshot", root, "--artifact", "../escape"], dir.path());
    assert_eq!(code(&refused), Some(64), "{}", stderr(&refused));
    assert_eq!(code(&run(&["snapshot", "missing"], dir.path())), Some(66));
    drop(lock);
}

/// The operations section explains a run checkpoint's records from what
/// the process table says now, without marking or rewriting the file: a
/// dispatch whose runtime outlived its loop is stalled (exit 1) until that
/// runtime ends, after which it is recoverable by the next loop start.
#[test]
fn doctor_explains_stalled_then_recoverable_operations_and_a_legacy_marker() {
    let dir = workspace();
    let root = dir.path().to_str().expect("utf8 root");
    let mut runtime = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a runtime that outlives the loop");
    let handle = magistr_core::workflow::runtime_handle::RuntimeHandle::observe(runtime.id())
        .expect("live handle");
    let mut exited = std::process::Command::new("true").spawn().expect("spawn");
    let gone = exited.id();
    exited.wait().expect("reaped");
    let run_key = format!("run-1-{gone}");
    let record = |local: &str, kind: &str, target: &str, outcome: &str, external: Value| {
        serde_json::json!({
            "attempt": { "command": { "run": { "project": "proj", "local": run_key }, "local": local }, "number": 1 },
            "kind": kind, "target": target, "outcome": outcome, "external": external,
        })
    };
    let checkpoint = serde_json::json!({
        "iteration": 0, "max_iterations": 5,
        "run": { "mode": "chunk_loop", "completed_attempts": 0, "max_iterations": 5,
                 "consecutive_no_commits": 0, "budget_total_usd": 0.0, "budget_limit_usd": null,
                 "terminal_reason": null, "last_settled_attempt": null },
        "run_id": { "project": "proj", "local": run_key },
        "operations": { "records": [
            record("0/attempt", "reservation", "attempt:0", "completed", Value::Null),
            record("0/dispatch", "provider_dispatch", "dispatch", "running", serde_json::json!(handle.to_string())),
        ] },
        "attempt_started": 0, "native_chunk_id": "chunk-a", "last_commit_sha": null,
        "last_status": "none", "spec_checked": 0, "spec_total": 1,
    });
    let state = dir.path().join(".magistr");
    std::fs::create_dir_all(&state).expect("state");
    let path = state.join("loop-state.json");
    let bytes = serde_json::to_vec_pretty(&checkpoint).expect("json");
    std::fs::write(&path, &bytes).expect("checkpoint");

    let stalled = run(&["doctor", root], dir.path());
    assert_eq!(code(&stalled), Some(1), "{}", stderr(&stalled));
    let out = stdout(&stalled);
    assert!(out.contains("1 stalled, 0 recoverable"), "{out}");
    assert!(
        out.contains(&format!(
            "stalled: attempt \"0/dispatch\" (\"provider_dispatch\") is owned by runtime pid {} which outlived the loop",
            runtime.id()
        )),
        "{out}"
    );
    assert!(
        out.contains("remediation: wait for that runtime to end"),
        "{out}"
    );
    assert!(out.contains(&format!("loop process {gone} gone")), "{out}");

    runtime.kill().expect("end the runtime");
    runtime.wait().expect("reap");
    let recoverable = run(&["doctor", root], dir.path());
    assert_eq!(code(&recoverable), Some(0), "{}", stderr(&recoverable));
    let out = stdout(&recoverable);
    assert!(out.contains("0 stalled, 1 recoverable"), "{out}");
    assert!(out.contains("recoverable: attempt \"0/dispatch\""), "{out}");
    assert!(out.contains("Git baseline"), "{out}");
    assert_eq!(
        std::fs::read(&path).expect("checkpoint"),
        bytes,
        "read, never rewritten"
    );

    // A legacy marker is an unhealthy owner finding naming the migration.
    let workbench = dir.path().join(".workbench");
    std::fs::create_dir_all(&workbench).expect("workbench");
    std::fs::write(
        workbench.join(ServerLock::FILE_NAME),
        br#"{"pid":0,"host":"elsewhere"}"#,
    )
    .expect("legacy marker");
    let legacy = run(&["doctor", root], dir.path());
    assert_eq!(code(&legacy), Some(1), "{}", stderr(&legacy));
    let out = stdout(&legacy);
    assert!(out.contains("legacy server record"), "{out}");
    assert!(out.contains("magistr workspace migrate"), "{out}");
    assert!(out.contains("no owner reachable"), "{out}");
}
