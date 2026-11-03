//! The composition boundary against its real adapters (tasks arch-14f and
//! arch-14-verification-check_results): an admitted Host writer over the native
//! workflow store — on the writer config production admits it on, carrying
//! this workspace's evidence capability — and an admitted `WorkspaceService`
//! over a disposable workspace, composed the way `magistr mcp` and the
//! loop compose them. No fake port, no MCP roundtrip, no second owner.
//!
//! Every completion here goes through `plan_complete_task` on the
//! writer — the verb the MCP tool, the owner command, the loop judge and
//! the cockpit's Complete action all land on — so what these tests prove
//! about refusal and acceptance holds for every route.

use std::path::{Path, PathBuf};

use magistr::application::AdmittedCheck;
use magistr::application::workflow_client::NativeEnginePort;
use magistr_cockpit::engine::EnginePort;
use magistr_engine::native::ProjectAuthority;
use magistr_engine::native::{
    CheckExecution, CheckResultSettlement, CheckSettlement, NativeConfig, NativeEngines,
    NativeReader, SourceFreshness, StepType, TaskStatus, WriterRole,
};
use magistr_workspace::{
    commands::{EditInput, EditOp},
    contracts::WorkspaceResponse,
    service::{WorkspaceOpenOptions, WorkspaceService},
    snapshot::MutationStatus,
};

const PROJECT: &str = "composition-integration";
const TASK: &str = "test";

/// A workspace with a project configuration naming three checks: the
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
        "checks": { "check": "true", "test": test_command, "lint": "true" },
        "format": { "extensions": ["rs"], "command": "true" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "reference_repos": [],
        "state_prefix": "p",
        "plan_backend": "native"
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
        .plan_add_task(
            TASK.into(),
            "The task under test".into(),
            String::new(),
            TaskStatus::ToDo,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("task");
    engines.plan_start_task(TASK).expect("in progress");
    (engines, config)
}

async fn owner(root: &Path) -> WorkspaceService {
    WorkspaceService::open(
        WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr")).with_network_override("0"),
    )
    .await
    .expect("admitted workspace")
}

fn goal(engines: &NativeEngines, step: &str) {
    goal_with(engines, step, vec![]);
}

fn goal_with(engines: &NativeEngines, step: &str, acceptance: Vec<String>) {
    engines
        .run_open_with_verification(
            "Compose".into(),
            acceptance,
            vec![],
            format!("task:{TASK}"),
            magistr_engine::native::VerificationMode::Full,
        )
        .expect("goal");
    engines
        .add_step(step.into(), "Verify".into(), StepType::Test, None)
        .expect("step");
    engines.start_step(step).expect("active step");
}

/// One check admitted and settled through the boundary, the way
/// `magistr_check` and the loop settle theirs.
fn check(engines: &NativeEngines, root: &Path, name: &str, code: i32) -> CheckSettlement {
    AdmittedCheck::admit(engines, root, None, name, "true".into(), true)
        .expect("admitted")
        .settle(engines, CheckExecution::Exited { code }, String::new())
        .expect("settled")
        .settlement
}

fn all_checks(engines: &NativeEngines, root: &Path) {
    for name in ["check", "test", "lint"] {
        assert!(matches!(
            check(engines, root, name, 0),
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

fn refusal(engines: &NativeEngines, step_ref: Option<&str>) -> String {
    engines
        .plan_complete_task(TASK, step_ref)
        .expect_err("completion refused")
        .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_check_admitted_with_source_observes_the_owners_write_and_lands_on_its_step() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, config) = host(&tmp, service.root());
    goal(&engines, "verify");

    let admitted =
        AdmittedCheck::admit(&engines, service.root(), None, "test", "true".into(), true)
            .expect("admitted with source");
    let identities = admitted.identities();
    // The workflow names the project, run, step and instance; the
    // workspace names the canonical root and the owner's generation —
    // side by side, and neither derived from the other.
    assert_eq!(identities.project_id, PROJECT);
    assert_eq!(identities.step_id, "verify");
    assert_eq!(identities.workspace.as_ref(), Some(&service.identity()));
    assert_eq!(
        identities.workspace.as_ref().and_then(|w| w.generation),
        Some(service.generation().get())
    );
    let admitted_revision = admitted.source().expect("observation").revision.clone();

    // The agent edits through the owner while the check runs.
    let seq = owner_edit(&service, "alpha", "beta").await;

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled");
    let check = settled.check().expect("applied on the admitted step");
    assert!(check.passed && check.verified);
    let check_result = settled.check_result();
    assert_eq!(
        check_result.source.as_ref().expect("source").revision,
        admitted_revision
    );
    assert_eq!(
        check_result.settlement,
        Some(CheckResultSettlement::Applied)
    );
    assert_eq!(
        check_result.freshness,
        Some(SourceFreshness::Changed {
            paths: vec!["notes.txt (Modified)".into()]
        }),
        "the owner's write must be observed"
    );
    // The workspace's own check_result says the same write was applied under
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
        "a mutation check_result revision and a source revision are different facts"
    );

    // Storage evidence: the check is in the native store the way the
    // agent's own check_step would leave it, visible to an unadmitted
    // reader, and the check_result is durable beside it.
    let reader = NativeReader::open_for_project(&config, PROJECT.into());
    let steps = reader.run_steps().unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].id, "verify");
    let checks = &steps[0].checks;
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].name, "test");
    assert!(checks[0].verified);
    let published = config
        .data_dir()
        .join("run/executions")
        .join(config.execution_context().key())
        .join("check_results")
        .join(PROJECT)
        .join(format!("{}.json", check_result.id));
    assert!(published.is_file(), "{}", published.display());

    // A check that saw the tree move under it is not evidence of the tree
    // as it is now: completion refuses it by name, and accepts the set
    // once every check has run against the settled tree.
    let refused = refusal(&engines, Some("step:verify"));
    assert!(
        refused.contains("test: the workspace changed while the check ran: notes.txt (Modified)"),
        "{refused}"
    );
    assert!(
        refused.contains("check: required check has no check_result"),
        "{refused}"
    );
    all_checks(&engines, service.root());
    let done = engines
        .plan_complete_task(TASK, Some("step:verify"))
        .expect("every required check is current");
    assert_eq!(done.value.status, TaskStatus::Done);
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_refuses_after_a_source_config_or_acceptance_change_until_the_checks_rerun() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, _) = host(&tmp, service.root());
    goal_with(&engines, "verify", vec!["it works".into()]);
    all_checks(&engines, service.root());

    // The source moved on through the owner after the checks passed.
    owner_edit(&service, "alpha", "beta").await;
    let refused = refusal(&engines, None);
    assert!(
        refused.contains("the source changed since the check ran"),
        "{refused}"
    );
    all_checks(&engines, service.root());

    // The check configuration changed: the config input moves the revision
    // and the contract names another command.
    std::fs::write(root.join(".magistr/project.json"), project_json("false")).expect("config");
    let refused = refusal(&engines, None);
    assert!(refused.contains("test: "), "{refused}");
    assert!(
        refused.contains("source changed") || refused.contains("not the configured"),
        "{refused}"
    );
    std::fs::write(root.join(".magistr/project.json"), project_json("true")).expect("config");
    all_checks(&engines, service.root());

    // The acceptance was edited mid-flight: same run, new revision.
    engines
        .run_open_with_verification(
            "Compose".into(),
            vec!["it works".into(), "and reads well".into()],
            vec![],
            format!("task:{TASK}"),
            magistr_engine::native::VerificationMode::Full,
        )
        .expect("edited in flight");
    let refused = refusal(&engines, None);
    assert!(refused.contains("acceptance changed"), "{refused}");
    all_checks(&engines, service.root());
    assert_eq!(
        engines
            .plan_complete_task(TASK, None)
            .expect("current again")
            .value
            .status,
        TaskStatus::Done
    );
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rolled_over_goal_leaves_the_result_stale_and_a_reused_name_has_no_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, config) = host(&tmp, service.root());
    goal(&engines, "verify");
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
    let check_result_id = admitted.check_result_id().to_owned();

    engines.run_finish(vec![], None).expect("shipped");
    goal(&engines, "verify");
    owner_edit(&service, "alpha", "gamma").await;

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "green".into())
        .expect("a stale settlement is an answer, not an error");
    match &settled.settlement {
        CheckSettlement::Stale {
            admission,
            outcome,
            check_result,
        } => {
            assert_eq!(admission.target, original);
            assert_ne!(
                admission.target.step_instance,
                engines
                    .run_admit_step(Some("verify"))
                    .expect("new instance")
                    .step_instance
            );
            assert!(outcome.passed);
            assert_eq!(check_result.value.id, check_result_id);
            assert_eq!(check_result.value.target.run_id, original.run_id);
            assert!(matches!(
                check_result.value.settlement,
                Some(CheckResultSettlement::Stale { .. })
            ));
            assert!(
                !check_result
                    .value
                    .freshness
                    .as_ref()
                    .expect("validated")
                    .is_unchanged()
            );
        }
        CheckSettlement::Applied { .. } => panic!("a reused step name must not collect it"),
    }
    assert!(
        engines
            .run_steps()
            .unwrap()
            .iter()
            .all(|step| step.checks.is_empty())
    );
    // The late check_result is durable under its own run; the new run's
    // `verify` cannot complete on it.
    let reader = NativeReader::open_for_project(&config, PROJECT.into());
    assert!(
        reader
            .run_steps()
            .unwrap()
            .iter()
            .all(|step| step.checks.is_empty())
    );
    let refused = refusal(&engines, Some("step:verify"));
    assert!(
        refused.contains("check: required check has no check_result on step `verify` in this run"),
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
async fn the_cockpit_complete_action_names_the_verification_step_and_refuses_without_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace(&tmp, "workspace");
    let service = owner(&root).await;
    let (engines, _) = host(&tmp, service.root());
    let mut port = NativeEnginePort::new(engines.clone());

    // No goal yet: the button is refused in words the stage can show.
    let refused = port.complete_task(TASK).expect_err("no evidence");
    assert!(refused.to_string().contains("no run goal"), "{refused}");

    goal(&engines, "verify");
    // A failed check, then a passing one: recovered, not outstanding.
    assert!(matches!(
        check(&engines, service.root(), "test", 1),
        CheckSettlement::Applied { .. }
    ));
    let refused = port.complete_task(TASK).expect_err("red check");
    assert!(
        refused
            .to_string()
            .contains("test: the latest attempt did not pass (exit 1)"),
        "{refused}"
    );
    all_checks(&engines, service.root());

    let done = port.complete_task(TASK).expect("current evidence");
    assert_eq!(done.value.status, magistr_engine::TaskStatus::Done);
    assert!(
        done.value.refs.iter().any(|r| r == "step:verify"),
        "the completion names the verification step it was decided on: {done:?}"
    );
    // The final summary tells the recovered failure apart from an
    // outstanding one.
    let report = engines.run_finish(vec![], None).expect("report");
    assert_eq!(report.value.recovered_failures, vec!["test (step: verify)"]);
    assert!(report.value.outstanding_failures.is_empty());
    assert!(
        !report
            .value
            .warnings
            .iter()
            .any(|w| w.starts_with("required checks failed:")),
        "{:?}",
        report.value.warnings
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
        .plan_set_status(TASK, TaskStatus::Done)
        .expect_err("no direct done");
    assert!(
        refused.to_string().contains("plan_complete_task"),
        "{refused}"
    );

    engines
        .run_open("Other".into(), vec![], vec![], "task:other".into())
        .expect("goal");
    engines
        .add_step("verify".into(), "Verify".into(), StepType::Test, None)
        .expect("step");
    engines.start_step("verify").expect("active");
    all_checks(&engines, service.root());
    let refused = refusal(&engines, Some("step:verify"));
    assert!(refused.contains("scoped to `task:other`"), "{refused}");
    service.shutdown().await.expect("clean shutdown");
}
