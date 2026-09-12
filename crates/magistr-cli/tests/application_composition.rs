//! The composition boundary against its real adapters (chunk arch-14f): an
//! admitted Host writer over the native workflow store and an admitted
//! `WorkspaceService` over a disposable workspace, composed the way
//! `magistr mcp-serve` and the loop compose them — no fake port, no MCP
//! roundtrip, no second owner.

use std::path::{Path, PathBuf};

use magistr::application::{AdmittedCheck, CheckEvidenceError};
use magistr_engine::tas::{
    CheckSettlement, NativeEngines, NativeReader, ProjectAuthority, TasConfig, TaskType, WriterRole,
};
use magistr_workspace::{
    commands::{EditInput, EditOp},
    contracts::WorkspaceResponse,
    service::{WorkspaceOpenOptions, WorkspaceService},
    snapshot::{ChangeKind, MutationStatus, SelectionPolicy, SnapshotError, SnapshotValidation},
};

const PROJECT: &str = "composition-integration";

fn workspace(tmp: &tempfile::TempDir, name: &str) -> PathBuf {
    let root = tmp.path().join(name);
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("source");
    root
}

fn host(tmp: &tempfile::TempDir) -> (NativeEngines, TasConfig) {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("project directory");
    let config = TasConfig::from_lookup(root, |_| None).with_data_dir(tmp.path().join("store"));
    let engines = ProjectAuthority::admit_for_project(&config, PROJECT.into(), WriterRole::Host)
        .expect("host admitted")
        .open();
    (engines, config)
}

async fn owner(root: &Path) -> WorkspaceService {
    WorkspaceService::open(
        WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr")).with_network_override("0"),
    )
    .await
    .expect("admitted workspace")
}

fn objective(engines: &NativeEngines, task: &str) {
    engines
        .ship_set_objective("Compose".into(), vec![], vec![], "chunk:test".into())
        .expect("objective");
    engines
        .ship_add_task(task.into(), "Verify".into(), TaskType::Test, None)
        .expect("task");
    engines.ship_start_task(task).expect("active task");
}

/// One edit through the admitted owner, the way every workspace client
/// writes; answers the journal sequence of the applied intent.
async fn owner_edit(service: &WorkspaceService, old: &str, new: &str) -> i64 {
    let session = service.new_session().expect("session");
    let mut input = EditInput::new(EditOp::ReplaceText);
    input.file = Some("notes.txt".into());
    input.old = Some(old.into());
    input.new = Some(new.into());
    input.auto_apply = true;
    match service.edit(&input, &session).await {
        WorkspaceResponse::Success { results, .. } => {
            let rows = results.rows();
            assert_eq!(rows.len(), 1);
            assert!(rows[0].applied);
            rows[0].journal_seq.expect("journaled")
        }
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("edit refused: {} {notes:?}", error.message)
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_check_admitted_with_source_observes_the_owners_write_and_lands_on_its_task() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, config) = host(&tmp);
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    objective(&engines, "verify");

    let admitted = AdmittedCheck::admit(
        &engines,
        service.root(),
        None,
        "test",
        "true".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect("admitted with source");
    let identities = admitted.identities();
    // The workflow names the project, run, task and instance; the
    // workspace names the canonical root and the owner's generation —
    // side by side, and neither derived from the other.
    assert_eq!(identities.project_id, PROJECT);
    assert_eq!(identities.task_id, "verify");
    assert_eq!(identities.workspace.as_ref(), Some(&service.identity()));
    assert_eq!(
        identities.workspace.as_ref().and_then(|w| w.generation),
        Some(service.generation().get())
    );
    let admitted_revision = admitted.source().expect("snapshot").revision();

    // The agent edits through the owner while the gate runs.
    let seq = owner_edit(&service, "alpha", "beta").await;

    let settled = admitted.settle(&engines, 0, "ok".into()).expect("settled");
    let check = settled.check().expect("applied on the admitted task");
    assert!(check.passed && check.verified);
    let source = settled.source.as_ref().expect("source evidence");
    assert_eq!(source.revision(), admitted_revision);
    match &source.validation {
        SnapshotValidation::Changed(changes) => {
            assert_eq!(changes.len(), 1, "{changes:?}");
            assert_eq!(changes[0].path, "notes.txt");
            assert_eq!(changes[0].kind, ChangeKind::Modified);
        }
        SnapshotValidation::Unchanged => panic!("the owner's write must be observed"),
    }
    // The workspace's own receipt says the same write was applied under
    // the generation the check named; nothing here invents a shared revision.
    let receipt = service
        .mutation_receipt(seq)
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(receipt.status, MutationStatus::Applied);
    assert_eq!(receipt.generation, Some(service.generation().get()));
    assert_ne!(
        receipt.revision.expect("applied revision"),
        source.revision(),
        "a mutation receipt revision and a source revision are different facts"
    );

    // Storage compatibility: the check is in the native store the way the
    // agent's own ship_check would leave it, visible to an unadmitted reader.
    let reader = NativeReader::open_for_project(&config, PROJECT.into());
    let checks = reader.ship_status()["checks"]
        .as_array()
        .cloned()
        .expect("checks");
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0]["name"], "test");
    assert_eq!(checks[0]["task_id"], "verify");
    assert_eq!(checks[0]["verified"], true);
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_owner_refuses_a_boundary_snapshot_of_another_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, _) = host(&tmp);
    let root = workspace(&tmp, "workspace");
    let other = workspace(&tmp, "other");
    let service = owner(&root).await;
    objective(&engines, "verify");

    let foreign = AdmittedCheck::admit(
        &engines,
        &other,
        None,
        "lint",
        "true".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect("admitted against the other checkout");
    let refused = service
        .validate_snapshot(foreign.source().expect("snapshot"))
        .expect_err("another checkout never substitutes");
    assert!(
        matches!(refused, SnapshotError::ForeignWorkspace { .. }),
        "{refused}"
    );
    // The boundary's own typed refusal for an unobservable root.
    let missing = AdmittedCheck::admit(
        &engines,
        &tmp.path().join("absent"),
        None,
        "lint",
        "true".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect_err("missing root");
    assert!(
        matches!(
            missing,
            CheckEvidenceError::Source(SnapshotError::RootMissing(_))
        ),
        "{missing}"
    );
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rolled_over_objective_leaves_the_result_stale_with_its_source_intact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, _) = host(&tmp);
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(
        &engines,
        service.root(),
        Some("verify"),
        "check",
        "false".into(),
        true,
        Some(SelectionPolicy::default()),
    )
    .expect("admitted");
    let original = admitted.target().clone();

    engines.ship_finalize(vec![], None).expect("shipped");
    objective(&engines, "verify");
    owner_edit(&service, "alpha", "gamma").await;

    let settled = admitted
        .settle(&engines, 1, "red".into())
        .expect("a stale settlement is an answer, not an error");
    match &settled.settlement {
        CheckSettlement::Stale { admission, outcome } => {
            assert_eq!(admission.target, original);
            assert_ne!(
                admission.target.task_instance,
                engines
                    .ship_admit_task(Some("verify"))
                    .expect("new instance")
                    .task_instance
            );
            assert_eq!(outcome.exit_code, Some(1));
            assert!(!outcome.passed);
        }
        CheckSettlement::Applied { .. } => panic!("a reused task name must not collect it"),
    }
    assert!(!settled.source.expect("source").is_unchanged());
    assert!(
        engines.ship_status()["checks"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    service.shutdown().await.expect("clean shutdown");
}
