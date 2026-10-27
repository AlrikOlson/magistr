//! Session admission through the real native workspace composition: replacement
//! owners fence every effect, while deliberately re-admitted identities retain
//! their durable undo history and must review fresh confirmation grants.
use std::{fs, path::Path};

use magistr_workspace::{
    commands::{EditInput, EditOp, FilesInput, FilesOp, ScriptInput, ScriptInterpreter, UndoInput},
    contracts::WorkspaceResponse,
    execution::RunInput,
    response::ResponseAdapter,
    service::{JournalListInput, WorkspaceOpenOptions, WorkspaceService, WorkspaceSession},
};
use serde::Serialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(dir.path().join(".git")).expect("repository marker");
    fs::create_dir_all(dir.path().join("src")).expect("source directory");
    fs::write(
        dir.path().join("src/app.ts"),
        "export function app(): number {\n  return 1;\n}\n",
    )
    .expect("source");
    fs::write(dir.path().join("pending.txt"), "reviewed bytes\n").expect("move source");
    // Confirmation fencing is exercised through the opt-in human gate.
    fs::create_dir_all(dir.path().join(".magistr")).expect("settings directory");
    fs::write(
        dir.path().join(".magistr/workspace.toml"),
        "[edit]\nconfirm_mode = \"elicit\"\n",
    )
    .expect("human confirmation mode");
    dir
}

async fn open(root: &Path) -> WorkspaceService {
    let interpreter = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/qjs/qjs-wasi.wasm");
    WorkspaceService::open(
        WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr"))
            .with_network_override("0")
            .with_interpreter(ScriptInterpreter::load(&interpreter).expect("pinned interpreter")),
    )
    .await
    .expect("admitted workspace")
}

fn envelope<R: Serialize>(response: WorkspaceResponse<R>) -> Value {
    ResponseAdapter::seal(&response)
        .expect("sealed response")
        .structured()
        .clone()
}

fn success<R: Serialize>(response: WorkspaceResponse<R>) -> Value {
    let result = envelope(response);
    assert_eq!(result["ok"], true, "{result}");
    result
}

fn refused<R: Serialize>(response: WorkspaceResponse<R>, code: &str) -> Value {
    let result = envelope(response);
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["error"]["code"], code, "{result}");
    result
}

fn edit(old: &str, new: &str) -> EditInput {
    let mut input = EditInput::new(EditOp::ReplaceText);
    input.file = Some("src/app.ts".into());
    input.old = Some(old.into());
    input.new = Some(new.into());
    input
}

fn create() -> FilesInput {
    let mut input = FilesInput::new(FilesOp::Create);
    input.to = Some("unwanted.txt".into());
    input
}

async fn journal(service: &WorkspaceService, session: &WorkspaceSession) -> Value {
    success(
        service
            .journal_list(&JournalListInput::default(), session)
            .await,
    )["results"]
        .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replacement_owner_fences_every_effect_before_journaling_and_readmits_undo_identity() {
    let dir = workspace();
    let first = open(dir.path()).await;
    let stale = first.admit_session("alice").expect("first admission");
    success(first.edit(&edit("return 1;", "return 2;"), &stale).await);
    first.shutdown().await.expect("first owner shutdown");

    let current = open(dir.path()).await;
    assert!(current.generation().get() > stale.generation().get());
    let alice = current
        .admit_session(stale.id())
        .expect("deliberate re-admission");
    let before = journal(&current, &alice).await;
    assert_eq!(before.as_array().expect("journal rows").len(), 1);
    assert_eq!(before[0]["sessionId"], "alice");
    refused(
        current.edit(&edit("return 2;", "return 9;"), &stale).await,
        "ENGINE_UNAVAILABLE",
    );
    refused(current.files(&create(), &stale).await, "ENGINE_UNAVAILABLE");
    refused(
        current.undo(&UndoInput::default(), &stale).await,
        "ENGINE_UNAVAILABLE",
    );
    refused(
        current
            .journal_list(&JournalListInput::default(), &stale)
            .await,
        "ENGINE_UNAVAILABLE",
    );
    refused(
        current
            .run(
                &RunInput::new("undeclared"),
                &stale,
                CancellationToken::new(),
            )
            .await,
        "ENGINE_UNAVAILABLE",
    );
    refused(
        current
            .script(&ScriptInput::new("return 1;"), &stale)
            .await
            .expect("admission precedes interpreter dispatch"),
        "ENGINE_UNAVAILABLE",
    );
    refused(
        current
            .run_plugin_tool("absent.plugin/tool", &json!({}), 4000, &stale)
            .await
            .expect("admission precedes plugin lookup"),
        "ENGINE_UNAVAILABLE",
    );
    assert_eq!(
        journal(&current, &alice).await,
        before,
        "refused calls leave the journal intact"
    );
    assert!(
        fs::read_to_string(dir.path().join("src/app.ts"))
            .expect("source")
            .contains("return 2;")
    );
    assert!(!dir.path().join("unwanted.txt").exists());

    let bob = current.admit_session("bob").expect("different session");
    let nothing = success(current.undo(&UndoInput::default(), &bob).await);
    assert_eq!(nothing["results"], json!([]));
    assert_eq!(journal(&current, &alice).await, before);
    success(current.undo(&UndoInput::default(), &alice).await);
    assert!(
        fs::read_to_string(dir.path().join("src/app.ts"))
            .expect("restored source")
            .contains("return 1;")
    );
    let after = journal(&current, &alice).await;
    assert_eq!(after.as_array().expect("journal rows").len(), 2);
    assert!(
        after
            .as_array()
            .expect("rows")
            .iter()
            .all(|row| row["sessionId"] == "alice")
    );
    current
        .shutdown()
        .await
        .expect("replacement owner shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_previous_generations_confirmation_fails_after_same_identity_readmission() {
    let dir = workspace();
    let first = open(dir.path()).await;
    let previous = first.admit_session("alice").expect("first admission");
    let mut request = FilesInput::new(FilesOp::Move);
    request.from = Some("pending.txt".into());
    request.to = Some("moved.txt".into());
    let preview = refused(first.files(&request, &previous).await, "CONFIRM_REQUIRED");
    let token = preview["error"]["confirmToken"]
        .as_str()
        .expect("token")
        .to_owned();
    assert!(token.starts_with("mgw2."));
    assert_eq!(journal(&first, &previous).await, json!([]));
    first.shutdown().await.expect("first owner shutdown");

    let current = open(dir.path()).await;
    let alice = current
        .admit_session(previous.id())
        .expect("same durable identity");
    assert_ne!(alice.generation(), previous.generation());
    request.confirm = Some(token.clone());
    refused(current.files(&request, &alice).await, "CONFIRM_MISMATCH");
    assert_eq!(
        fs::read_to_string(dir.path().join("pending.txt")).expect("unchanged source"),
        "reviewed bytes\n"
    );
    assert!(!dir.path().join("moved.txt").exists());
    assert_eq!(journal(&current, &alice).await, json!([]));

    request.confirm = None;
    let fresh = refused(current.files(&request, &alice).await, "CONFIRM_REQUIRED");
    let fresh_token = fresh["error"]["confirmToken"]
        .as_str()
        .expect("fresh token");
    assert_ne!(fresh_token, token);
    let bob = current.admit_session("bob").expect("different session");
    request.confirm = Some(fresh_token.to_owned());
    refused(current.files(&request, &bob).await, "CONFIRM_MISMATCH");
    success(current.files(&request, &alice).await);
    assert!(!dir.path().join("pending.txt").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("moved.txt")).expect("moved bytes"),
        "reviewed bytes\n"
    );
    assert!(
        !journal(&current, &alice)
            .await
            .as_array()
            .expect("journal rows")
            .is_empty()
    );
    current
        .shutdown()
        .await
        .expect("replacement owner shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn equal_generations_do_not_admit_another_workspaces_session() {
    let a = workspace();
    let b = workspace();
    let first = open(a.path()).await;
    let second = open(b.path()).await;
    assert_eq!(
        first.generation(),
        second.generation(),
        "independent fresh roots start at the same generation"
    );
    let foreign = first
        .admit_session("alice")
        .expect("first workspace session");
    let local = second
        .admit_session("alice")
        .expect("second workspace session");
    refused(
        second.files(&create(), &foreign).await,
        "ENGINE_UNAVAILABLE",
    );
    assert_eq!(journal(&second, &local).await, json!([]));
    assert!(!b.path().join("unwanted.txt").exists());
    first.shutdown().await.expect("first shutdown");
    second.shutdown().await.expect("second shutdown");
}
