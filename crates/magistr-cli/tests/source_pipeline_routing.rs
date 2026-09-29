//! Magistr's own source writes through the admitted workspace pipeline
//! (chunks arch-14f2 and audit-260912-04), against the real adapters: a
//! `HostedWorkspace` the way `magistr run` admits one (the real worker
//! binary, the owner endpoint served), its `WorkspaceSourceWriter`, the
//! `WorkspaceAdmission` every host composes, and the ACP `LocalEffectPort`
//! the session's writes arrive through. No fake port, no MCP roundtrip, no
//! second owner — and no direct write when the workspace is not admitted.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use magistr::application::{HostedWorkspace, WorkspaceAdmission};
use magistr_acp::AcpEvent;
use magistr_acp::capabilities::LocalEffectPort;
use magistr_application::effects::{
    EffectAuthorizer, EffectError, EffectFuture, EffectPort, EffectRequest, EffectResponse,
    SourceRefusal, SourceRoute,
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

fn port(root: &Path, source: SourceRoute) -> (LocalEffectPort, Events) {
    let (events, rx) = mpsc::unbounded_channel();
    (LocalEffectPort::new(root.to_path_buf(), events, source), rx)
}

/// The route a host that admitted `hosted` hands its session.
fn admitted(hosted: &HostedWorkspace) -> SourceRoute {
    SourceRoute::Admitted(hosted.source_writer().expect("session"))
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
    let (port, mut rx) = port(&root, admitted(&hosted));
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
    let (port, mut rx) = port(&root, admitted(&hosted));

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
    let (port, _rx) = port(&root, admitted(&hosted));

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

/// The write a refused host's session asks for: refused before any
/// permission decision, nothing on disk, nothing in the owner's journal and
/// no receipt — and the session goes on reading.
async fn refused_write_leaves_the_tree_alone(
    root: &Path,
    admission: &WorkspaceAdmission,
    expected: &SourceRefusal,
) {
    let route = admission.source_route();
    assert_eq!(route.refusal(), Some(expected), "{}", admission.report());
    let (port, mut rx) = port(root, route);
    assert!(
        !port.capabilities().write_text_file,
        "a host without an admitted writer advertises no write capability"
    );
    let decision = Decision::allowing();
    let target = root.join("refused.txt");
    let refused = port
        .execute(write(&target, "never\n"), decision.clone())
        .await;
    assert!(
        matches!(&refused, Err(EffectError::Unavailable(reason)) if reason.starts_with("source write refused")),
        "{refused:?}"
    );
    assert!(!target.exists(), "nothing lands directly");
    assert!(
        decision.seen.lock().expect("lock").is_empty(),
        "no permission is asked for a write that cannot land"
    );
    let events = fs_write_events(&mut rx);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["route"], "refused");
    assert_eq!(events[0]["refusal"], expected.kind());

    // The session keeps operating: the read is authorized and answered.
    let read = port
        .execute(
            EffectRequest::ReadTextFile {
                session_id: "s".into(),
                path: root.join("notes.txt").display().to_string(),
                line: None,
                limit: None,
            },
            decision.clone(),
        )
        .await
        .expect("read");
    assert_eq!(
        read,
        EffectResponse::Text("alpha".into()),
        "the read handler joins lines without a trailing newline"
    );
    assert_eq!(decision.seen.lock().expect("lock").len(), 1);
    drop(port);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_workspace_refuses_the_second_hosts_writes_and_invents_no_receipt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let hosted = hosted(&root).await;
    let root = hosted.service().root().to_path_buf();
    let before = hosted
        .service()
        .source_snapshot(SelectionPolicy::default())
        .expect("snapshot");

    // A second host on the same checkout admits nothing: no second owner,
    // no attachment to write through — and no direct write either.
    let second = WorkspaceAdmission::admit_with(options(&root)).await;
    assert!(second.hosted().is_none(), "a live owner is never doubled");
    assert!(matches!(
        WorkspaceService::open(options(&root)).await,
        Err(WorkspaceOpenError::Locked(_))
    ));
    refused_write_leaves_the_tree_alone(&root, &second, &SourceRefusal::OwnerElsewhere).await;

    // To the owner nothing happened: no journal row, no receipt, and the
    // tree is exactly what it was.
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
        SnapshotValidation::Unchanged
    );

    second.shutdown().await.expect("a refusal releases nothing");
    hosted.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_marker_needing_migration_refuses_admission_and_the_write() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    let state = root.join(".workbench");
    std::fs::create_dir(&state).expect("state");
    // A retained legacy owner's record: it takes no OS lock, and a native
    // host must never migrate it on its own.
    let marker = format!(r#"{{"pid":{},"host":"legacy-host"}}"#, std::process::id());
    std::fs::write(state.join("server.lock"), &marker).expect("legacy owner");

    let admission = WorkspaceAdmission::admit_with(options(&root)).await;
    let WorkspaceAdmission::Refused(SourceRefusal::NotAdmitted(reason)) = &admission else {
        panic!("a legacy marker is a refusal: {}", admission.report());
    };
    assert!(reason.contains("migration"), "{reason}");
    let root = root.canonicalize().expect("canonical");
    refused_write_leaves_the_tree_alone(
        &root,
        &admission,
        &SourceRefusal::NotAdmitted(reason.clone()),
    )
    .await;

    // The refusal opened nothing and preserved the record.
    assert_eq!(
        std::fs::read_to_string(state.join("server.lock")).expect("preserved"),
        marker
    );
    for forbidden in ["journal.db", "index.db", "keys"] {
        assert!(
            !state.join(forbidden).exists(),
            "refusal opened {forbidden}"
        );
    }
    admission.shutdown().await.expect("nothing to release");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invalid_configuration_refuses_admission_and_the_write() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);
    // A protected-paths pattern the policy does not support.
    std::fs::write(
        root.join("workbench.toml"),
        "[security]\nprotected_paths = ['secrets/*.pem']\n",
    )
    .expect("configuration");

    let admission = WorkspaceAdmission::admit_with(options(&root)).await;
    let WorkspaceAdmission::Refused(SourceRefusal::NotAdmitted(reason)) = &admission else {
        panic!(
            "an invalid configuration is a refusal: {}",
            admission.report()
        );
    };
    assert!(reason.contains("protected_paths"), "{reason}");
    let root = root.canonicalize().expect("canonical");
    refused_write_leaves_the_tree_alone(
        &root,
        &admission,
        &SourceRefusal::NotAdmitted(reason.clone()),
    )
    .await;
    assert!(!root.join("refused.txt").exists());
    admission.shutdown().await.expect("nothing to release");
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

/// A loop takes its source route once and keeps it for every iteration.
/// Releasing the admission together with that route shuts the owner down
/// in order; a plain shutdown beside the live route is refused, which is
/// how every loop used to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_loop_releases_its_route_with_the_admission_and_the_owner_shuts_down() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp);

    let admission = WorkspaceAdmission::admit_with(options(&root)).await;
    let route = admission.source_route();
    assert!(route.is_admitted());
    let refused = admission.shutdown().await;
    assert!(
        refused.is_err(),
        "a live route keeps the service referenced"
    );
    drop(route);

    let admission = WorkspaceAdmission::admit_with(options(&root)).await;
    let route = admission.source_route();
    assert!(route.is_admitted(), "{}", admission.report());
    admission
        .release(route)
        .await
        .expect("the route goes first, then the owner shuts down");
    let again = WorkspaceAdmission::admit_with(options(&root)).await;
    assert!(again.hosted().is_some(), "{}", again.report());
    again.shutdown().await.expect("clean shutdown");
}
