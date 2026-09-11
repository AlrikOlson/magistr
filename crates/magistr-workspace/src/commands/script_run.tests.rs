use super::*;
use crate::{
    commands::{ScriptInput, ScriptInterpreter, ScriptRow, ScriptStatus, WriteAction, WriteReport},
    config::ResolveOptions,
    confirmation::ConfirmationSeal,
    contracts::{ErrorKind, WorkspaceResponse},
    extensions::{ExtensionLane, LaneFuture, PluginGuestPolicy, WasiLaneOutcome},
    jail::WorkspaceKeyStore,
    journal::{JournalDb, JournalDbOptions},
    transaction::FormatterOptions,
    worker::{WorkerGuestEnd, WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};
use serde_json::{Value, json};
use std::{fs, sync::Mutex};

/// Worker-free harness: `.txt` targets take the unsupported verification
/// lane and glob/read need no parser, so the protocol, error mapping and the
/// queue pipeline are exercised without a guest or a parser child. The real
/// QuickJS guest lives in the magistr-cli integration suite.
struct Harness {
    root: tempfile::TempDir,
    policy: WorkspacePolicy,
    worker: WorkerSupervisor,
    journal: JournalDb,
    seal: ConfirmationSeal,
}

impl Harness {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".git")).unwrap();
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root.path())).unwrap();
        let worker =
            WorkerSupervisor::new("/unavailable-worker".into(), WorkerOptions::default()).unwrap();
        let journal = JournalDb::open(JournalDbOptions::new(
            root.path().join(".workbench/journal.db"),
        ))
        .unwrap();
        let key = WorkspaceKeyStore::new(root.path().join(".workbench"))
            .ensure()
            .unwrap();
        let harness = Self {
            root,
            policy,
            worker,
            journal,
            seal: ConfirmationSeal::new(&key, 1),
        };
        harness.write("notes/a.txt", "one TODO\ntwo TODO\n");
        harness.write("notes/b.txt", "no marker\n");
        harness.write("config.json", "{\"count\":42}\n");
        harness
    }

    fn write(&self, path: &str, content: &str) {
        let file = self.root.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }

    fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.path().join(path)).ok()
    }

    fn commands(&mut self) -> WorkspaceCommands<'_> {
        WorkspaceCommands::new(
            &self.policy,
            &self.worker,
            &mut self.journal,
            &self.seal,
            None,
            FormatterOptions::default(),
            "suite",
        )
        .unwrap()
    }
}

fn frame(nonce: &str, body: Value) -> String {
    format!("@@WB[{nonce}]{body}[{nonce}]WB@@")
}

type RoundHandler =
    Box<dyn Fn(usize, &[Value], &str) -> Result<WasiLaneOutcome, String> + Send + Sync>;

/// A scripted guest: the handler sees the round's journal and nonce.
struct FakeGuest {
    handler: RoundHandler,
    rounds: Mutex<usize>,
    policies: Mutex<Vec<PluginGuestPolicy>>,
}

impl FakeGuest {
    fn new(
        handler: impl Fn(usize, &[Value], &str) -> Result<WasiLaneOutcome, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            handler: Box::new(handler),
            rounds: Mutex::new(0),
            policies: Mutex::new(Vec::new()),
        }
    }

    /// A guest that prints one frame and exits 0.
    fn emitting(body: impl Fn(usize, &[Value]) -> Value + Send + Sync + 'static) -> Self {
        Self::new(move |round, journal, nonce| Ok(exit_ok(frame(nonce, body(round, journal)))))
    }
}

fn exit_ok(stdout: String) -> WasiLaneOutcome {
    WasiLaneOutcome {
        end: WorkerGuestEnd::Ok,
        fuel_used: 10,
        exit_code: Some(0),
        stdout,
        stderr: String::new(),
    }
}

impl ExtensionLane for FakeGuest {
    fn run_wasi(
        &self,
        _guest: Vec<u8>,
        argv: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, WasiLaneOutcome> {
        self.policies.lock().unwrap().push(policy);
        let mut rounds = self.rounds.lock().unwrap();
        *rounds += 1;
        let round = *rounds;
        assert_eq!(argv[0], "qjs");
        assert_eq!(argv[1], "-e");
        assert!(argv[2].contains("(function(workspace){"));
        let journal: Vec<Value> = serde_json::from_str(&argv[3]).unwrap();
        let answer = (self.handler)(round, &journal, &argv[4]);
        Box::pin(async move {
            answer.map_err(|message| {
                crate::contracts::WorkspaceError::new(ErrorKind::EngineUnavailable, message)
            })
        })
    }
}

fn interpreter() -> ScriptInterpreter {
    ScriptInterpreter::from_bytes(b"fake qjs".to_vec())
}

async fn run(
    h: &mut Harness,
    input: ScriptInput,
    guest: &FakeGuest,
) -> WorkspaceResponse<ScriptRow> {
    h.commands().script(&input, &interpreter(), guest).await
}

fn refused(response: WorkspaceResponse<ScriptRow>) -> (ErrorKind, String, Vec<String>) {
    match response {
        WorkspaceResponse::Failure { error, notes, .. } => {
            (error.kind, error.message, notes.unwrap_or_default())
        }
        WorkspaceResponse::Success { results, .. } => {
            panic!("expected refusal, got {:?}", results.rows())
        }
    }
}

fn ok(response: WorkspaceResponse<ScriptRow>) -> (Vec<ScriptRow>, Vec<String>) {
    match response {
        WorkspaceResponse::Success { results, notes, .. } => {
            (results.rows().to_vec(), notes.unwrap_or_default())
        }
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("expected success, got {error} {notes:?}")
        }
    }
}

fn script_row(rows: &[ScriptRow]) -> ScriptResult {
    match &rows[0] {
        ScriptRow::Script(row) => row.clone(),
        ScriptRow::Write(_) => panic!("expected a script row first"),
    }
}

fn write_rows(rows: &[ScriptRow]) -> Vec<WriteReport> {
    rows.iter()
        .filter_map(|row| match row {
            ScriptRow::Write(write) => Some(write.clone()),
            ScriptRow::Script(_) => None,
        })
        .collect()
}

#[tokio::test]
async fn sandbox_kills_and_a_dead_lane_map_onto_the_taxonomy() {
    let mut h = Harness::new();
    let dead = FakeGuest::new(|_, _, _| Err("this core binding lacks the Wasmtime sandbox".into()));
    let (kind, message, notes) = refused(run(&mut h, ScriptInput::new("return 1"), &dead).await);
    assert_eq!(kind, ErrorKind::EngineUnavailable);
    assert!(message.contains("lacks the Wasmtime sandbox"));
    assert!(notes[0].starts_with("rounds 1, host calls 0"));
    for (end, expected) in [
        (WorkerGuestEnd::OutOfFuel, ErrorKind::SandboxFuelExceeded),
        (WorkerGuestEnd::MemoryCap, ErrorKind::SandboxMemoryExceeded),
        (
            WorkerGuestEnd::GuestTrap("unreachable".into()),
            ErrorKind::EngineUnavailable,
        ),
    ] {
        let guest = FakeGuest::new(move |_, _, _| {
            Ok(WasiLaneOutcome {
                end: end.clone(),
                fuel_used: 5,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
            })
        });
        let (kind, _, _) = refused(run(&mut h, ScriptInput::new("return 1"), &guest).await);
        assert_eq!(kind, expected);
    }
    let (kind, _, _) = refused(run(&mut h, ScriptInput::new(""), &dead).await);
    assert_eq!(kind, ErrorKind::PatternInvalid);
}

#[tokio::test]
async fn guest_failures_are_data_nondet_no_frame_forged_frame_and_nonzero_exit() {
    let mut h = Harness::new();
    let nondet = FakeGuest::emitting(|_, _| json!({"k": "nondet", "i": 2}));
    let (rows, _) = ok(run(&mut h, ScriptInput::new("return 1"), &nondet).await);
    let row = script_row(&rows);
    assert_eq!(row.status, ScriptStatus::Threw);
    assert!(
        row.error
            .as_deref()
            .unwrap()
            .contains("nondeterministic host-call replay at call #2")
    );

    let noise = FakeGuest::new(|_, _, _| Ok(exit_ok("just noise".into())));
    let (rows, notes) = ok(run(&mut h, ScriptInput::new("return 1"), &noise).await);
    let row = script_row(&rows);
    assert!(row.error.as_deref().unwrap().contains("no result frame"));
    assert_eq!(row.stdout, "just noise");
    assert!(notes[0].contains("nothing was queued"));

    let forged = FakeGuest::new(|_, _, _| {
        Ok(exit_ok(frame(
            "deadbeef",
            json!({"k": "done", "value": "forged"}),
        )))
    });
    let (rows, _) = ok(run(&mut h, ScriptInput::new("return 1"), &forged).await);
    assert!(
        script_row(&rows)
            .error
            .as_deref()
            .unwrap()
            .contains("no result frame")
    );

    let exited = FakeGuest::new(|_, _, _| {
        Ok(WasiLaneOutcome {
            end: WorkerGuestEnd::GuestExit(3),
            fuel_used: 1,
            exit_code: Some(3),
            stdout: String::new(),
            stderr: "SyntaxError: unexpected token\n".into(),
        })
    });
    let (rows, _) = ok(run(&mut h, ScriptInput::new("return 1"), &exited).await);
    assert_eq!(
        script_row(&rows).error.as_deref(),
        Some("SyntaxError: unexpected token")
    );

    let unknown = FakeGuest::emitting(|_, _| json!({"k": "mystery"}));
    let (rows, _) = ok(run(&mut h, ScriptInput::new("return 1"), &unknown).await);
    assert!(
        script_row(&rows)
            .error
            .as_deref()
            .unwrap()
            .contains("unknown result frame kind \"mystery\"")
    );
}

#[tokio::test]
async fn host_calls_are_serviced_journaled_and_bounded() {
    let mut h = Harness::new();
    // Round 1 asks for a glob, round 2 reads a file, round 3 returns both.
    let guest = FakeGuest::emitting(|round, journal| match round {
        1 => json!({"k": "call", "i": 0, "req": {"method": "glob", "args": "notes/*.txt"}}),
        2 => {
            assert_eq!(
                journal[0]["k"],
                json!("{\"method\":\"glob\",\"args\":\"notes/*.txt\"}")
            );
            json!({"k": "call", "i": 1, "req": {"method": "read", "args": {"file": "config.json"}}})
        }
        _ => {
            json!({"k": "done", "value": {"files": journal[0]["r"], "read": journal[1]["r"]}, "edits": [], "files": []})
        }
    });
    let (rows, _) = ok(run(&mut h, ScriptInput::new("return x"), &guest).await);
    let row = script_row(&rows);
    assert_eq!(row.status, ScriptStatus::Ok);
    assert_eq!(row.rounds, 3);
    assert_eq!(row.host_calls, 2);
    assert_eq!(row.fuel_used, 30);
    let value = row.value.unwrap();
    assert_eq!(value["files"], json!(["notes/a.txt", "notes/b.txt"]));
    assert_eq!(value["read"]["ok"], json!(true));
    let text: String = value["read"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|unit| unit["kind"] == "lines")
        .map(|unit| unit["text"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(text, "{\"count\":42}");

    let exec = FakeGuest::emitting(
        |_, _| json!({"k": "call", "i": 0, "req": {"method": "exec", "args": "rm -rf /"}}),
    );
    let (kind, message, _) = refused(run(&mut h, ScriptInput::new("return 1"), &exec).await);
    assert_eq!(kind, ErrorKind::SandboxCapDenied);
    assert!(message.contains("unknown host method \"exec\""));

    let escape = FakeGuest::emitting(
        |_, _| json!({"k": "call", "i": 0, "req": {"method": "read", "args": {"file": "../../etc/passwd"}}}),
    );
    let (kind, message, _) = refused(run(&mut h, ScriptInput::new("return 1"), &escape).await);
    assert_eq!(kind, ErrorKind::SandboxCapDenied);
    assert!(message.contains("workspace.read(\"../../etc/passwd\")"));

    let malformed = FakeGuest::emitting(
        |_, _| json!({"k": "call", "i": 0, "req": {"method": "search", "args": {"query": ""}}}),
    );
    let (kind, message, _) = refused(run(&mut h, ScriptInput::new("return 1"), &malformed).await);
    assert_eq!(kind, ErrorKind::SandboxCapDenied);
    assert!(message.contains("non-empty query"));

    let greedy = FakeGuest::emitting(
        |round, _| json!({"k": "call", "i": round - 1, "req": {"method": "glob", "args": "**/*.txt"}}),
    );
    let (kind, message, notes) = refused(run(&mut h, ScriptInput::new("return 1"), &greedy).await);
    assert_eq!(kind, ErrorKind::SandboxFuelExceeded);
    assert!(message.contains("32 workspace calls"));
    assert!(notes[0].contains("host calls 32"));
}

fn done(value: Value, edits: Vec<Value>, files: Vec<Value>) -> FakeGuest {
    FakeGuest::emitting(
        move |_, _| json!({"k": "done", "value": value, "edits": edits, "files": files}),
    )
}

#[tokio::test]
async fn queued_edits_preview_by_default_and_a_throw_discards_the_queue() {
    let mut h = Harness::new();
    let guest = done(
        json!(true),
        vec![json!({"file": "notes/a.txt", "old": "TODO", "new": "DONE"})],
        vec![],
    );
    let (rows, notes) = ok(run(&mut h, ScriptInput::new("q"), &guest).await);
    let row = script_row(&rows);
    assert_eq!(row.status, ScriptStatus::Ok);
    assert_eq!(row.queued_writes, 1);
    let writes = write_rows(&rows);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].path, "notes/a.txt");
    assert!(!writes[0].applied);
    assert!(writes[0].diff.contains("+one DONE"));
    assert!(notes.iter().any(|n| n.contains("dryRun")));
    assert_eq!(h.read("notes/a.txt").unwrap(), "one TODO\ntwo TODO\n");

    let threw = FakeGuest::emitting(
        |_, _| json!({"k": "done", "threw": "halfway failure", "edits": [{"file": "notes/a.txt", "old": "TODO", "new": "GONE"}], "files": []}),
    );
    let (rows, notes) = ok(run(&mut h, ScriptInput::new("q"), &threw).await);
    let row = script_row(&rows);
    assert_eq!(row.status, ScriptStatus::Threw);
    assert_eq!(row.error.as_deref(), Some("halfway failure"));
    assert_eq!(row.queued_writes, 1);
    assert!(write_rows(&rows).is_empty());
    assert!(notes[0].contains("discarded"));
    assert_eq!(h.read("notes/a.txt").unwrap(), "one TODO\ntwo TODO\n");
}

#[tokio::test]
async fn file_ops_queue_create_mkdir_and_delete_previews_and_refuse_the_rest() {
    let mut h = Harness::new();
    let guest = done(
        json!("planned"),
        vec![],
        vec![
            json!({"op": "create", "to": "src/generated/marker.txt"}),
            json!({"op": "mkdir", "to": "src/new-area"}),
            json!({"op": "create", "to": "notes/a.txt"}),
            json!({"op": "delete", "from": "notes/b.txt"}),
        ],
    );
    let (rows, notes) = ok(run(&mut h, ScriptInput::new("q"), &guest).await);
    assert_eq!(script_row(&rows).value, Some(json!("planned")));
    let writes = write_rows(&rows);
    let mut paths: Vec<_> = writes.iter().map(|w| w.path.clone()).collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "notes/b.txt",
            "src/generated/marker.txt",
            "src/new-area/.gitkeep"
        ]
    );
    let deleted = writes.iter().find(|w| w.path == "notes/b.txt").unwrap();
    assert_eq!(deleted.action, WriteAction::Deleted);
    assert!(!deleted.applied);
    assert!(notes.iter().any(|n| n.contains("create never overwrites")));
    assert!(h.read("src/generated/marker.txt").is_none());
    assert!(h.read("notes/b.txt").is_some());

    let moved = done(
        json!(1),
        vec![],
        vec![json!({"op": "move", "from": "a", "to": "b"})],
    );
    let (kind, message, _) = refused(run(&mut h, ScriptInput::new("q"), &moved).await);
    assert_eq!(kind, ErrorKind::SandboxCapDenied);
    assert!(message.contains("move/copy are not queueable"));
    let missing = done(
        json!(1),
        vec![],
        vec![json!({"op": "delete", "from": "ghost.txt"})],
    );
    assert_eq!(
        refused(run(&mut h, ScriptInput::new("q"), &missing).await).0,
        ErrorKind::FileNotFound
    );
    let escaping = done(
        json!(1),
        vec![json!({"file": "../outside.txt", "old": "a", "new": "b"})],
        vec![],
    );
    assert_eq!(
        refused(run(&mut h, ScriptInput::new("q"), &escaping).await).0,
        ErrorKind::SandboxCapDenied
    );
    let protected = done(
        json!(1),
        vec![json!({"file": ".workbench/journal.db", "old": "a", "new": "b"})],
        vec![],
    );
    assert_eq!(
        refused(run(&mut h, ScriptInput::new("q"), &protected).await).0,
        ErrorKind::SandboxCapDenied
    );
    let bad_occurrence = done(
        json!(1),
        vec![json!({"file": "notes/a.txt", "old": "TODO", "new": "x", "occurrence": 0})],
        vec![],
    );
    assert_eq!(
        refused(run(&mut h, ScriptInput::new("q"), &bad_occurrence).await).0,
        ErrorKind::SandboxCapDenied
    );
}

#[tokio::test]
async fn applying_is_always_confirm_gated_and_the_token_flow_applies_the_queue() {
    let mut h = Harness::new();
    let guest = done(
        json!("queued"),
        vec![json!({"file": "notes/a.txt", "old": "one TODO", "new": "one DONE"})],
        vec![],
    );
    let mut input = ScriptInput::new("q");
    input.dry_run = false;
    let (kind, _, notes) = refused(run(&mut h, input.clone(), &guest).await);
    let ErrorKind::ConfirmRequired { confirm_token } = kind else {
        panic!("gated");
    };
    assert!(notes[0].contains("script return value: \"queued\""));
    assert_eq!(h.read("notes/a.txt").unwrap(), "one TODO\ntwo TODO\n");

    let mut confirmed = input.clone();
    confirmed.confirm = Some(confirm_token.clone());
    let (rows, notes) = ok(run(&mut h, confirmed, &guest).await);
    assert_eq!(script_row(&rows).value, Some(json!("queued")));
    let writes = write_rows(&rows);
    assert!(writes[0].applied);
    assert!(writes[0].journal_seq.is_some());
    assert!(notes.iter().any(|n| n.contains("applied through")));
    assert_eq!(h.read("notes/a.txt").unwrap(), "one DONE\ntwo TODO\n");
    let ops = h.journal.list(0).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].tool, "script");

    // A changed intent (different source) cannot reuse the token.
    h.write("notes/a.txt", "one TODO\ntwo TODO\n");
    let mut other = input.clone();
    other.source = "different".into();
    other.confirm = Some(confirm_token.clone());
    assert_eq!(
        refused(run(&mut h, other, &guest).await).0,
        ErrorKind::ConfirmMismatch
    );
    // Changed underneath so the anchor still matches: the sealed plan differs.
    h.write("notes/a.txt", "changed underneath\none TODO\n");
    let mut stale = input.clone();
    stale.confirm = Some(confirm_token.clone());
    assert_eq!(
        refused(run(&mut h, stale, &guest).await).0,
        ErrorKind::ConfirmMismatch
    );
    // Changed underneath so the anchor is gone: the fresh hash is answered.
    h.write("notes/a.txt", "nothing here\n");
    let mut gone = input.clone();
    gone.confirm = Some(confirm_token);
    assert!(matches!(
        refused(run(&mut h, gone, &guest).await).0,
        ErrorKind::StaleAnchor { .. }
    ));
    // autoApply is the trusted-harness bypass.
    h.write("notes/a.txt", "one TODO\n");
    let mut auto = input.clone();
    auto.auto_apply = true;
    let (rows, _) = ok(run(&mut h, auto, &guest).await);
    assert!(write_rows(&rows)[0].applied);
    assert_eq!(h.read("notes/a.txt").unwrap(), "one DONE\n");
}

#[tokio::test]
async fn a_round_tripped_queue_is_nothing_to_change_and_caps_clamp_downward() {
    let mut h = Harness::new();
    let guest = done(
        json!(0),
        vec![
            json!({"file": "notes/a.txt", "old": "TODO", "new": "X"}),
            json!({"file": "notes/a.txt", "old": "X", "new": "TODO"}),
        ],
        vec![],
    );
    let mut input = ScriptInput::new("q");
    input.fuel = Some(u64::MAX);
    input.memory_cap_bytes = Some(usize::MAX);
    let (rows, notes) = ok(run(&mut h, input.clone(), &guest).await);
    assert_eq!(rows.len(), 1);
    assert!(notes.iter().any(|n| n.contains("nothing to change")));
    assert_eq!(
        guest.policies.lock().unwrap()[0],
        PluginGuestPolicy::ceiling()
    );
    let mut small = input.clone();
    small.fuel = Some(50);
    small.memory_cap_bytes = Some(65_536);
    ok(run(&mut h, small, &guest).await);
    assert_eq!(
        guest.policies.lock().unwrap()[1],
        PluginGuestPolicy {
            fuel: 50,
            memory_cap_bytes: 65_536
        }
    );
    let mut confirmed = input;
    confirmed.confirm = Some("mgw1.anything".into());
    assert_eq!(
        refused(run(&mut h, confirmed, &guest).await).0,
        ErrorKind::ConfirmMismatch
    );
}
