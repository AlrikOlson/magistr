//! The composition boundary against its real adapters (chunks arch-14f and
//! arch-14-verification-receipts): an admitted Host writer over the native
//! workflow store — on the writer config production admits it on, carrying
//! this workspace's evidence capability — and an admitted `WorkspaceService`
//! over a disposable workspace, composed the way `magistr mcp-serve` and the
//! loop compose them. No fake port, no MCP roundtrip, no second owner.
//!
//! Every completion here goes through `roadmap_complete_chunk` on the
//! writer — the verb the MCP tool, the owner command, the loop judge and
//! the cockpit's Complete action all land on — so what these tests prove
//! about refusal and acceptance holds for every route.

use std::path::{Path, PathBuf};

use magistr::application::AdmittedCheck;
use magistr::application::workflow_client::NativeEnginePort;
use magistr_cockpit::engine::EnginePort;
use magistr_engine::native::ProjectAuthority;
use magistr_engine::native::{
    CheckExecution, CheckSettlement, ChunkStatus, NativeConfig, NativeEngines, NativeReader,
    ReceiptSettlement, SourceFreshness, TaskType, WriterRole,
};
use magistr_workspace::{
    commands::{EditInput, EditOp},
    contracts::WorkspaceResponse,
    service::{WorkspaceOpenOptions, WorkspaceService},
    snapshot::MutationStatus,
};

const PROJECT: &str = "composition-integration";
const CHUNK: &str = "test";

/// A workspace with a project configuration naming three gates: the
/// evidence capability resolves the contracts from it, and the snapshot
/// hashes it as a config input.
fn workspace(tmp: &tempfile::TempDir, name: &str) -> PathBuf {
    let root = tmp.path().join(name);
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::create_dir_all(root.join(".magistr")).expect("config directory");
    std::fs::write(root.join(".magistr/project.json"), project_json("true")).expect("config");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("source");
    root
}

fn project_json(test_command: &str) -> String {
    serde_json::json!({
        "project": { "name": "p", "description": "", "type": "cli" },
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": { "check": "true", "test": test_command, "lint": "true" },
        "format": { "extensions": ["rs"], "command": "true" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "reference_repos": [],
        "state_prefix": "p",
        "roadmap_backend": "think-and-ship"
    })
    .to_string()
}

/// The Host writer, admitted the way `admit_native_host` admits it: on a
/// writer config whose evidence capability observes `root`.
fn host(tmp: &tempfile::TempDir, root: &Path) -> (NativeEngines, NativeConfig) {
    let config = magistr_core::evidence::writer_config(root, |_| None)
        .with_data_dir(tmp.path().join("store"));
    let engines = ProjectAuthority::admit_for_project(&config, PROJECT.into(), WriterRole::Host)
        .expect("host admitted")
        .open();
    engines
        .roadmap_add_chunk(
            CHUNK.into(),
            "The chunk under test".into(),
            String::new(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("chunk");
    engines.roadmap_start_chunk(CHUNK).expect("in progress");
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
    objective_with(engines, task, vec![]);
}

fn objective_with(engines: &NativeEngines, task: &str, acceptance: Vec<String>) {
    engines
        .ship_set_objective(
            "Compose".into(),
            acceptance,
            vec![],
            format!("chunk:{CHUNK}"),
        )
        .expect("objective");
    engines
        .ship_add_task(task.into(), "Verify".into(), TaskType::Test, None)
        .expect("task");
    engines.ship_start_task(task).expect("active task");
}

/// One gate admitted and settled through the boundary, the way
/// `magistr_gate` and the loop settle theirs.
fn gate(engines: &NativeEngines, root: &Path, name: &str, code: i32) -> CheckSettlement {
    AdmittedCheck::admit(engines, root, None, name, "true".into(), true)
        .expect("admitted")
        .settle(engines, CheckExecution::Exited { code }, String::new())
        .expect("settled")
        .settlement
}

fn all_gates(engines: &NativeEngines, root: &Path) {
    for name in ["check", "test", "lint"] {
        assert!(matches!(
            gate(engines, root, name, 0),
            CheckSettlement::Applied { .. }
        ));
    }
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

fn refusal(engines: &NativeEngines, ship_ref: Option<&str>) -> String {
    engines
        .roadmap_complete_chunk(CHUNK, ship_ref)
        .expect_err("completion refused")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_check_admitted_with_source_observes_the_owners_write_and_lands_on_its_task() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, config) = host(&tmp, service.root());
    objective(&engines, "verify");

    let admitted =
        AdmittedCheck::admit(&engines, service.root(), None, "test", "true".into(), true)
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
    let admitted_revision = admitted.source().expect("observation").revision.clone();

    // The agent edits through the owner while the gate runs.
    let seq = owner_edit(&service, "alpha", "beta").await;

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled");
    let check = settled.check().expect("applied on the admitted task");
    assert!(check.passed && check.verified);
    let receipt = settled.receipt();
    assert_eq!(
        receipt.source.as_ref().expect("source").revision,
        admitted_revision
    );
    assert_eq!(receipt.settlement, Some(ReceiptSettlement::Applied));
    assert_eq!(
        receipt.freshness,
        Some(SourceFreshness::Changed {
            paths: vec!["notes.txt (Modified)".into()]
        }),
        "the owner's write must be observed"
    );
    // The workspace's own receipt says the same write was applied under
    // the generation the check named; nothing here invents a shared revision.
    let mutation = service
        .mutation_receipt(seq)
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(mutation.status, MutationStatus::Applied);
    assert_eq!(mutation.generation, Some(service.generation().get()));
    assert_ne!(
        mutation.revision.expect("applied revision"),
        admitted_revision,
        "a mutation receipt revision and a source revision are different facts"
    );

    // Storage evidence: the check is in the native store the way the
    // agent's own ship_check would leave it, visible to an unadmitted
    // reader, and the receipt is durable beside it.
    let reader = NativeReader::open_for_project(&config, PROJECT.into());
    let tasks = reader.ship_tasks().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "verify");
    let checks = &tasks[0].checks;
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].name, "test");
    assert!(checks[0].verified);
    let published = config
        .data_dir()
        .join("ship/executions")
        .join(config.execution_context().key())
        .join("receipts")
        .join(PROJECT)
        .join(format!("{}.json", receipt.id));
    assert!(published.is_file(), "{}", published.display());

    // A gate that saw the tree move under it is not evidence of the tree
    // as it is now: completion refuses it by name, and accepts the set
    // once every gate has run against the settled tree.
    let refused = refusal(&engines, Some("task:verify"));
    assert!(
        refused.contains("test: the workspace changed while the gate ran: notes.txt (Modified)"),
        "{refused}"
    );
    assert!(
        refused.contains("check: required gate has no receipt"),
        "{refused}"
    );
    all_gates(&engines, service.root());
    let done = engines
        .roadmap_complete_chunk(CHUNK, Some("task:verify"))
        .expect("every required gate is current");
    assert_eq!(done.status, ChunkStatus::Done);
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_refuses_after_a_source_config_or_acceptance_change_until_the_gates_rerun() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, _) = host(&tmp, service.root());
    objective_with(&engines, "verify", vec!["it works".into()]);
    all_gates(&engines, service.root());

    // The source moved on through the owner after the gates passed.
    owner_edit(&service, "alpha", "beta").await;
    let refused = refusal(&engines, None);
    assert!(
        refused.contains("the source changed since the gate ran"),
        "{refused}"
    );
    all_gates(&engines, service.root());

    // The gate configuration changed: the config input moves the revision
    // and the contract names another command.
    std::fs::write(root.join(".magistr/project.json"), project_json("false")).expect("config");
    let refused = refusal(&engines, None);
    assert!(refused.contains("test: "), "{refused}");
    assert!(
        refused.contains("source changed") || refused.contains("not the configured"),
        "{refused}"
    );
    std::fs::write(root.join(".magistr/project.json"), project_json("true")).expect("config");
    all_gates(&engines, service.root());

    // The acceptance was edited mid-flight: same run, new revision.
    engines
        .ship_set_objective(
            "Compose".into(),
            vec!["it works".into(), "and reads well".into()],
            vec![],
            format!("chunk:{CHUNK}"),
        )
        .expect("edited in flight");
    let refused = refusal(&engines, None);
    assert!(refused.contains("acceptance changed"), "{refused}");
    all_gates(&engines, service.root());
    assert_eq!(
        engines
            .roadmap_complete_chunk(CHUNK, None)
            .expect("current again")
            .status,
        ChunkStatus::Done
    );
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rolled_over_objective_leaves_the_result_stale_and_a_reused_name_has_no_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, config) = host(&tmp, service.root());
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(
        &engines,
        service.root(),
        Some("verify"),
        "check",
        "true".into(),
        true,
    )
    .expect("admitted");
    let original = admitted.target().clone();
    let receipt_id = admitted.receipt_id().to_owned();

    engines.ship_finalize(vec![], None).expect("shipped");
    objective(&engines, "verify");
    owner_edit(&service, "alpha", "gamma").await;

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "green".into())
        .expect("a stale settlement is an answer, not an error");
    match &settled.settlement {
        CheckSettlement::Stale {
            admission,
            outcome,
            receipt,
        } => {
            assert_eq!(admission.target, original);
            assert_ne!(
                admission.target.task_instance,
                engines
                    .ship_admit_task(Some("verify"))
                    .expect("new instance")
                    .task_instance
            );
            assert!(outcome.passed);
            assert_eq!(receipt.id, receipt_id);
            assert_eq!(receipt.target.run_id, original.run_id);
            assert!(matches!(
                receipt.settlement,
                Some(ReceiptSettlement::Stale { .. })
            ));
            assert!(
                !receipt
                    .freshness
                    .as_ref()
                    .expect("validated")
                    .is_unchanged()
            );
        }
        CheckSettlement::Applied { .. } => panic!("a reused task name must not collect it"),
    }
    assert!(
        engines
            .ship_tasks()
            .unwrap()
            .iter()
            .all(|task| task.checks.is_empty())
    );
    // The late receipt is durable under its own run; the new run's
    // `verify` cannot complete on it.
    let reader = NativeReader::open_for_project(&config, PROJECT.into());
    assert!(
        reader
            .ship_tasks()
            .unwrap()
            .iter()
            .all(|task| task.checks.is_empty())
    );
    let refused = refusal(&engines, Some("task:verify"));
    assert!(
        refused.contains("check: required gate has no receipt on task `verify` in this run"),
        "{refused}"
    );
    let refused = refusal(&engines, None);
    assert!(
        refused.contains("no check was recorded in run"),
        "{refused}"
    );
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cockpit_complete_action_names_the_verification_task_and_refuses_without_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, _) = host(&tmp, service.root());
    let mut port = NativeEnginePort::new(engines.clone());

    // No objective yet: the button is refused in words the stage can show.
    let refused = port.complete_chunk(CHUNK).expect_err("no evidence");
    assert!(refused.contains("no ship objective"), "{refused}");

    objective(&engines, "verify");
    // A failed gate, then a passing one: recovered, not outstanding.
    assert!(matches!(
        gate(&engines, service.root(), "test", 1),
        CheckSettlement::Applied { .. }
    ));
    let refused = port.complete_chunk(CHUNK).expect_err("red gate");
    assert!(
        refused.contains("test: the latest attempt did not pass (exit 1)"),
        "{refused}"
    );
    all_gates(&engines, service.root());

    let done = port.complete_chunk(CHUNK).expect("current evidence");
    assert_eq!(done.status, magistr_engine::ChunkStatus::Done);
    assert!(
        done.cross_refs.iter().any(|r| r == "task:verify"),
        "the completion names the verification task it was decided on: {done:?}"
    );
    // The final summary tells the recovered failure apart from an
    // outstanding one.
    let report = engines.ship_finalize(vec![], None).expect("report");
    assert_eq!(report.recovered_failures, vec!["test (task: verify)"]);
    assert!(report.outstanding_failures.is_empty());
    assert!(
        !report
            .warnings
            .iter()
            .any(|w| w.starts_with("required checks failed:")),
        "{:?}",
        report.warnings
    );
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_plain_status_route_and_a_foreign_scope_are_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, _) = host(&tmp, service.root());
    let refused = engines
        .roadmap_set_status(CHUNK, ChunkStatus::Done)
        .expect_err("no direct done");
    assert!(refused.contains("roadmap_complete_chunk"), "{refused}");

    engines
        .ship_set_objective("Other".into(), vec![], vec![], "chunk:other".into())
        .expect("objective");
    engines
        .ship_add_task("verify".into(), "Verify".into(), TaskType::Test, None)
        .expect("task");
    engines.ship_start_task("verify").expect("active");
    all_gates(&engines, service.root());
    let refused = refusal(&engines, Some("task:verify"));
    assert!(refused.contains("scoped to `chunk:other`"), "{refused}");
    service.shutdown().await.expect("clean shutdown");
}
