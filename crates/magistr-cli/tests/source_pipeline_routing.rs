//! Magistr's own source writes through the admitted workspace pipeline
//! (chunk arch-14f2), against the real adapters: a `HostedWorkspace` the
//! way `magistr run` admits one (the real worker binary, the owner endpoint
//! served), its `WorkspaceSourceWriter`, and the ACP `LocalEffectPort` the
//! session's writes arrive through. No fake port, no MCP roundtrip, no
//! second owner.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use magistr::application::HostedWorkspace;
use magistr_acp::AcpEvent;
use magistr_acp::capabilities::LocalEffectPort;
use magistr_application::effects::{
    EffectAuthorizer, EffectError, EffectFuture, EffectPort, EffectRequest, EffectResponse,
};
use magistr_workspace::{
    contracts::WorkspaceResponse,
    diagnostics::JournalRow,
    service::{JournalListInput, WorkspaceOpenError, WorkspaceOpenOptions, WorkspaceService},
    snapshot::{ChangeKind, MutationStatus, SelectionPolicy, SnapshotValidation},
};
use tokio::sync::mpsc;

/// A repository with one valid Rust source and one text note.
fn workspace(tmp: &tempfile::TempDir) -> PathBuf {
    let root = tmp.path().join("workspace");
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(root.join("src/lib.rs"), "pub fn one() -> u32 {\n    1\n}\n").expect("source");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("note");
    root
}

fn options(root: &Path) -> WorkspaceOpenOptions {
    WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr")).with_network_override("0")
}

async fn hosted(root: &Path) -> HostedWorkspace {
    HostedWorkspace::admit_with(options(root))
        .await
        .expect("admissible")
        .expect("no other owner")
}

/// The session's permission decision, recorded.
struct Decision {
    allow: bool,
    seen: Mutex<Vec<EffectRequest>>,
}

impl Decision {
    fn allowing() -> Arc<Self> {
        Arc::new(Self {
            allow: true,
            seen: Mutex::new(Vec::new()),
        })
    }

    fn denying() -> Arc<Self> {
        Arc::new(Self {
            allow: false,
            seen: Mutex::new(Vec::new()),
        })
    }
}

impl EffectAuthorizer for Decision {
    fn authorize(&self, request: EffectRequest) -> EffectFuture<'_, ()> {
        Box::pin(async move {
            self.seen.lock().expect("lock").push(request);
            if self.allow {
                Ok(())
            } else {
                Err(EffectError::Denied("policy refused the write".into()))
            }
        })
    }

    fn is_cancelled(&self) -> bool {
        false
    }
}

type Events = mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>;

fn port(root: &Path, hosted: Option<&HostedWorkspace>) -> (LocalEffectPort, Events) {
    let (events, rx) = mpsc::unbounded_channel();
    let mut port = LocalEffectPort::new(root.to_path_buf(), events);
    if let Some(hosted) = hosted {
        port = port.with_source_writer(hosted.source_writer().expect("session"));
    }
    (port, rx)
}

fn write(path: &Path, content: &str) -> EffectRequest {
    EffectRequest::WriteTextFile {
        session_id: "s".into(),
        path: path.display().to_string(),
        content: content.into(),
    }
}

fn fs_write_events(rx: &mut Events) -> Vec<serde_json::Value> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            Ok(AcpEvent::System { subtype, data }) if subtype == "fs_write" => Some(data),
            _ => None,
        })
        .collect()
}

/// Every journaled op of the workspace, newest first.
async fn journal(service: &WorkspaceService) -> Vec<JournalRow> {
    let session = service.new_session().expect("session");
    match service
        .journal_list(&JournalListInput::default(), &session)
        .await
    {
        WorkspaceResponse::Success { results, .. } => results.rows().to_vec(),
        WorkspaceResponse::Failure { error, .. } => panic!("journal: {}", error.message),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_authorized_write_lands_through_the_pipeline_with_a_journal_intent_and_a_receipt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let hosted = hosted(&root).await;
    let root = hosted.service().root().to_path_buf();
    let before = hosted
        .service()
        .source_snapshot(SelectionPolicy::default())
        .expect("snapshot");
    let (port, mut rx) = port(&root, Some(&hosted));
    let decision = Decision::allowing();

    let target = root.join("src/new.rs");
    let response = port
        .execute(
            write(&target, "pub fn two() -> u32 {\n    2\n}\n"),
            decision.clone(),
        )
        .await
        .expect("written");
    assert_eq!(response, EffectResponse::Written);
    assert_eq!(
        decision.seen.lock().expect("lock").len(),
        1,
        "authorized first"
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("landed"),
        "pub fn two() -> u32 {\n    2\n}\n",
        "exact bytes, unformatted"
    );

    // The effect's own event names the route and the intent.
    let events = fs_write_events(&mut rx);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["route"], "workspace");
    let seq = events[0]["journal_seq"].as_i64().expect("journal intent");
    let revision = events[0]["revision"].as_str().expect("revision").to_owned();

    // The workspace's truthful receipt says the same intent applied under
    // this owner's generation, with the revision the event carried.
    let receipt = hosted
        .service()
        .mutation_receipt(seq)
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(receipt.status, MutationStatus::Applied);
    assert_eq!(receipt.tool, "write");
    assert_eq!(receipt.generation, Some(hosted.generation()));
    assert_eq!(receipt.revision.as_deref(), Some(revision.as_str()));
    assert_eq!(receipt.files.len(), 1);

    // One journal row, the write's; and the source revision moved.
    let rows = journal(hosted.service()).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].tool, "write");
    assert_eq!(rows[0].seq, seq);
    let validation = hosted
        .service()
        .validate_snapshot(&before)
        .expect("validated");
    assert_eq!(
        validation,
        SnapshotValidation::Changed(vec![magistr_workspace::snapshot::SourceChange {
            path: "src/new.rs".into(),
            kind: ChangeKind::Added,
        }])
    );
    let after = hosted
        .service()
        .source_snapshot(SelectionPolicy::default())
        .expect("snapshot");
    assert_ne!(after.revision(), before.revision());
    assert_ne!(
        after.revision(),
        revision,
        "a source revision and a mutation receipt revision are different facts"
    );

    drop(port);
    hosted.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_denied_write_never_reaches_the_pipeline_and_journals_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let hosted = hosted(&root).await;
    let root = hosted.service().root().to_path_buf();
    let (port, mut rx) = port(&root, Some(&hosted));

    let target = root.join("src/refused.rs");
    let refused = port
        .execute(write(&target, "pub fn no() {}\n"), Decision::denying())
        .await;
    assert!(
        matches!(refused, Err(EffectError::Denied(_))),
        "{refused:?}"
    );
    assert!(!target.exists());
    assert!(fs_write_events(&mut rx).is_empty());
    assert!(journal(hosted.service()).await.is_empty());

    drop(port);
    hosted.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_the_pipeline_refuses_is_the_effects_answer_in_its_words() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let hosted = hosted(&root).await;
    let root = hosted.service().root().to_path_buf();
    let (port, _rx) = port(&root, Some(&hosted));

    // A syntax error into a valid source: parse verification refuses it.
    let target = root.join("src/lib.rs");
    let refused = port
        .execute(write(&target, "pub fn broken( {\n"), Decision::allowing())
        .await;
    assert!(
        matches!(&refused, Err(EffectError::Failed(reason)) if reason.contains("syntax error")),
        "{refused:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("kept"),
        "pub fn one() -> u32 {\n    1\n}\n"
    );

    // The state directory is protected on every route.
    let protected = root.join(".workbench/journal.db");
    let refused = port
        .execute(write(&protected, "x"), Decision::allowing())
        .await;
    assert!(
        matches!(refused, Err(EffectError::Denied(_))),
        "{refused:?}"
    );
    assert!(journal(hosted.service()).await.is_empty());

    drop(port);
    hosted.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_an_admitted_workspace_the_effect_writes_directly_and_says_so() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let hosted = hosted(&root).await;
    let root = hosted.service().root().to_path_buf();

    // A second host on the same checkout admits nothing: no second owner,
    // no attachment to write through, and the effect keeps its direct write.
    let second = HostedWorkspace::admit_with(options(&root))
        .await
        .expect("admissible");
    assert!(second.is_none(), "a live owner is never doubled");
    assert!(matches!(
        WorkspaceService::open(options(&root)).await,
        Err(WorkspaceOpenError::Locked(_))
    ));
    let before = hosted
        .service()
        .source_snapshot(SelectionPolicy::default())
        .expect("snapshot");

    let (port, mut rx) = port(&root, None);
    let target = root.join("direct.txt");
    port.execute(write(&target, "direct\n"), Decision::allowing())
        .await
        .expect("written directly");
    assert_eq!(
        std::fs::read_to_string(&target).expect("landed"),
        "direct\n"
    );
    let events = fs_write_events(&mut rx);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["route"], "direct");
    assert!(
        events[0]["note"]
            .as_str()
            .expect("note")
            .contains("unjournaled")
    );

    // To the owner that write is external: no journal row, no receipt,
    // only the snapshot's word that the tree moved.
    assert!(journal(hosted.service()).await.is_empty());
    assert!(
        hosted
            .service()
            .mutation_receipt(1)
            .await
            .expect("read")
            .is_none()
    );
    assert_eq!(
        hosted
            .service()
            .validate_snapshot(&before)
            .expect("validated"),
        SnapshotValidation::Changed(vec![magistr_workspace::snapshot::SourceChange {
            path: "direct.txt".into(),
            kind: ChangeKind::Added,
        }])
    );

    drop(port);
    hosted.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn external_writes_stay_unjournaled_and_are_reported_by_snapshot_validation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let hosted = hosted(&root).await;
    let root = hosted.service().root().to_path_buf();
    let before = hosted
        .service()
        .source_snapshot(SelectionPolicy::default())
        .expect("snapshot");

    // An editor, a provider CLI or git touching the tree behind the owner.
    std::fs::write(root.join("notes.txt"), "edited elsewhere\n").expect("editor");
    std::fs::write(root.join("src/extra.rs"), "pub fn extra() {}\n").expect("provider");
    std::fs::remove_file(root.join("src/lib.rs")).expect("git checkout");

    assert!(journal(hosted.service()).await.is_empty());
    assert!(
        hosted
            .service()
            .mutation_receipt(1)
            .await
            .expect("read")
            .is_none(),
        "nothing external ever gets a receipt"
    );
    let SnapshotValidation::Changed(mut changes) = hosted
        .service()
        .validate_snapshot(&before)
        .expect("validated")
    else {
        panic!("external writes must be observed")
    };
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    let observed: Vec<(String, ChangeKind)> = changes
        .into_iter()
        .map(|change| (change.path, change.kind))
        .collect();
    assert_eq!(
        observed,
        vec![
            ("notes.txt".to_owned(), ChangeKind::Modified),
            ("src/extra.rs".to_owned(), ChangeKind::Added),
            ("src/lib.rs".to_owned(), ChangeKind::Removed),
        ]
    );

    hosted.shutdown().await.expect("clean shutdown");
}
