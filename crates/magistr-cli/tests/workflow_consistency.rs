//! One workflow owner serves shared in-process capabilities and attached commands.
//! Lease-free observers refresh persisted state as its store stamps change.
//! Duplicate project or source ownership is refused across all host kinds,
//! both within this process and across real processes.

#![cfg(unix)]

use std::io;
use std::path::{Path, PathBuf};

use magistr::application::HostedWorkspace;
use magistr_engine::native::{CallerIdentity, OwnerClient, OwnerCommand, OwnerReply, PlanCommand};
use magistr_engine::{
    NativeConfig, NativeEngines, NativeReader, OwnerEndpoint, ProjectAuthority, StepType,
    TaskStatus, WriterRole,
};
use magistr_workflow::signal::domain::SignalKind;
use magistr_workspace::service::WorkspaceOpenOptions;

const PROJECT: &str = "consistency";

/// A store under a tempdir is too deep for `sun_path`: the endpoint lands
/// in the runtime directory, and `/tmp` always fits.
fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    NativeConfig::from_lookup(root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_runtime_dir("/tmp")
}

fn admit(cfg: &NativeConfig, role: WriterRole) -> io::Result<ProjectAuthority> {
    ProjectAuthority::admit_for_project(cfg, PROJECT.into(), role)
}

fn writer(cfg: &NativeConfig, role: WriterRole) -> NativeEngines {
    admit(cfg, role).expect("admitted").open()
}

fn reader(cfg: &NativeConfig) -> NativeReader {
    NativeReader::open_for_project(cfg, PROJECT.into())
}

fn attached(cfg: &NativeConfig) -> OwnerClient {
    OwnerClient::attach_with(
        cfg,
        PROJECT.into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        OwnerClient::DEADLINE,
    )
    .expect("attached to the host")
}

fn add_task(engines: &NativeEngines, id: &str) {
    engines
        .plan_add_task(
            id.into(),
            format!("Task {id}"),
            String::new(),
            TaskStatus::ToDo,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("add task");
}

fn add_task_command(id: &str) -> OwnerCommand {
    OwnerCommand::Plan(PlanCommand::AddTask {
        id: id.into(),
        title: format!("Task {id}"),
        name: String::new(),
        status: TaskStatus::ToDo,
        priority: 100,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        content: None,
        feature: None,
    })
}

fn task_ids(reader: &NativeReader) -> Vec<String> {
    let mut ids: Vec<String> = reader
        .plan_tasks()
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    ids.sort();
    ids
}

fn checks_on(reader: &NativeReader, step: &str) -> Vec<String> {
    reader
        .run_steps()
        .unwrap()
        .into_iter()
        .filter(|t| t.id == step)
        .flat_map(|t| t.checks)
        .map(|check| check.name)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_attached_command_is_current_on_every_capability_and_observer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let endpoint = OwnerEndpoint::serve(host.clone()).await.expect("endpoint");
    let service = host.clone();
    let observer = reader(&cfg);
    // Both capabilities and the observer opened on an empty store.
    assert!(service.plan_tasks().unwrap().is_empty() && observer.plan_tasks().unwrap().is_empty());

    // The attached client's command runs on the host's engines: read-your-
    // writes for the owner, and on disk at once.
    let mut client = attached(&cfg);
    let OwnerReply::Task(task) = client
        .command(add_task_command("via-attached"))
        .expect("command")
    else {
        panic!("a task comes back");
    };
    assert_eq!(task.value.id, "via-attached");
    assert!(
        host.plan_task("via-attached").unwrap().is_some(),
        "the owner is current"
    );
    assert_eq!(task_ids(&reader(&cfg)), vec!["via-attached".to_owned()]);

    // The service view acts through the same writer. Its mutation preserves
    // the acknowledged record and becomes visible to persisted observers.
    assert!(
        service.plan_task("via-attached").unwrap().is_some(),
        "a query on the shared capability sees the acknowledged write"
    );
    add_task(&service, "via-service");
    assert_eq!(
        task_ids(&service.reader()),
        vec!["via-attached".to_owned(), "via-service".to_owned()],
        "the writer's memory is the merged file after its own save"
    );
    assert_eq!(
        task_ids(&reader(&cfg)),
        vec!["via-attached".to_owned(), "via-service".to_owned()],
        "the locked merge-on-save erased nothing"
    );
    // The observer opened before both writes and refreshes the current file.
    assert_eq!(
        task_ids(&observer),
        vec!["via-attached".to_owned(), "via-service".to_owned()]
    );
    assert!(host.plan_task("via-service").unwrap().is_some());
    // A transition on the host validates against the current record, so
    // starting the task the service just retired is refused with the
    // current status, not carried out on a stale pending copy.
    service
        .plan_retire_task("via-attached", "superseded".into())
        .expect("obsolete");
    let refused = host
        .plan_start_task("via-attached")
        .expect_err("retired on disk");
    assert!(refused.to_string().contains("replaced"), "{refused}");
    assert_eq!(
        host.plan_task("via-attached").unwrap().map(|c| c.status),
        Some(TaskStatus::Retired)
    );

    drop(client);
    drop(endpoint);
}

#[test]
fn shared_run_capabilities_and_observers_retain_a_recorded_gate() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let service = host.clone();
    let observer = reader(&cfg);

    // The host opens the cycle: a goal with one step, started.
    host.run_open("cycle".into(), vec![], vec![], "task:c1".into())
        .expect("goal");
    host.add_step("verify".into(), "verify".into(), StepType::Test, None)
        .expect("step");
    host.start_step("verify").expect("start");

    // The service view records a gate on the same admitted step.
    let check = service
        .run_record_gate(
            Some("verify"),
            "lint",
            "cargo clippy".into(),
            0,
            String::new(),
            true,
            None,
        )
        .expect("the service sees the host's step");
    assert!(check.value.verified && check.value.passed);
    assert_eq!(
        service.run_active_step_id().unwrap().as_deref(),
        Some("verify")
    );

    // Completing through another capability preserves the recorded check.
    let completed = host.finish_step("verify", vec![]).expect("complete");
    assert_eq!(
        completed
            .value
            .checks
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["lint"],
        "the host's verb answered the current step"
    );
    assert_eq!(checks_on(&reader(&cfg), "verify"), vec!["lint".to_owned()]);
    assert_eq!(
        host.run_steps()
            .unwrap()
            .iter()
            .filter(|step| step.status == magistr_workflow::run::domain::step::StepStatus::Done)
            .count(),
        1
    );

    // Queries read through as well: the service's status answers the
    // completed step the host wrote, and a reader opened on the empty
    // store answers the whole cycle — no reload on either.
    assert_eq!(
        service
            .run_steps()
            .unwrap()
            .iter()
            .filter(|step| step.status == magistr_workflow::run::domain::step::StepStatus::Done)
            .count(),
        1,
        "a query on the shared capability observes the current cycle"
    );
    assert_eq!(checks_on(&observer, "verify"), vec!["lint".to_owned()]);
    assert_eq!(
        observer
            .run_steps()
            .unwrap()
            .iter()
            .filter(|step| step.status == magistr_workflow::run::domain::step::StepStatus::Done)
            .count(),
        1
    );
    assert_eq!(
        observer
            .snapshot()
            .run
            .current()
            .unwrap()
            .value
            .steps()
            .len(),
        1
    );
}

#[test]
fn shared_signal_capabilities_promote_once_and_observers_refresh() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let service = host.clone();
    let observer = reader(&cfg);
    assert!(host.signal_list().unwrap().is_empty() && observer.signal_list().unwrap().is_empty());

    let captured = service
        .signal_capture(SignalKind::Idea, "alice".into(), "read through".into())
        .expect("capture");
    assert_eq!(
        host.signal(&captured.value.id)
            .unwrap()
            .map(|s| s.body)
            .as_deref(),
        Some("read through"),
        "the host's query observes the service's capture"
    );
    assert_eq!(observer.signal_status().unwrap()["counts"]["total"], 1);

    // A decision through the host capability is visible to the service view.
    host.signal_research(&captured.value.id, "worth it".into(), 0.9, vec![], None)
        .expect("research");
    let promoted = host.signal_promote(&captured.value.id).expect("promote");
    let task_id = promoted
        .signal
        .value
        .refs
        .iter()
        .find_map(|r| r.strip_prefix("task:"))
        .expect("a task back-reference")
        .to_owned();
    assert!(
        service.plan_task(&task_id).unwrap().is_some(),
        "the promoted task is current on the shared capability"
    );
    // The service's own promote of the same signal is the same promotion:
    // it observes the promoted record and creates nothing.
    let again = service
        .signal_promote(&captured.value.id)
        .expect("idempotent");
    assert_eq!(again.signal.value.refs, promoted.signal.value.refs);
    assert_eq!(observer.plan_tasks().unwrap().len(), 1);
}

#[test]
fn a_second_project_writer_is_refused_in_process_and_the_lease_ends_with_the_last_clone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let service = host.clone();
    for role in [WriterRole::Host, WriterRole::Service] {
        let refused = admit(&cfg, role).expect_err("a second project writer");
        assert_eq!(
            refused.kind(),
            io::ErrorKind::WouldBlock,
            "{role:?}: {refused}"
        );
    }
    // Readers and clones hold no lease of their own.
    let observer = reader(&cfg);
    let host_clone = host.clone();
    drop(host);
    assert_eq!(
        admit(&cfg, WriterRole::Host)
            .expect_err("clone keeps it")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(host_clone);
    drop(service);
    drop(observer);
    admit(&cfg, WriterRole::Host).expect("released with the last writer clone");
    admit(&cfg, WriterRole::Service).expect("either role may acquire the released project");
}

fn workspace_root(tmp: &tempfile::TempDir) -> PathBuf {
    let root = tmp.path().join("workspace");
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("note");
    root
}

fn options(root: &Path) -> WorkspaceOpenOptions {
    // Ownership and teardown still include an active watcher, without waiting
    // for the system FSEvents daemon during concurrent lifecycle tests.
    WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr"))
        .with_network_override("0")
        .with_session_flags(
            serde_json::from_value(serde_json::json!({"watcher.mode": "poll"})).unwrap(),
        )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_hosted_workspace_on_one_root_admits_nothing_while_the_first_lives() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = workspace_root(&tmp);
    let first = HostedWorkspace::admit_with(options(&root))
        .await
        .expect("admissible")
        .expect("the first owner");
    let generation = first.generation();
    assert!(
        HostedWorkspace::admit_with(options(&root))
            .await
            .expect("a held lock is not an error")
            .is_none(),
        "no second owner in one process"
    );
    first.shutdown().await.expect("ordered shutdown");
    let next = HostedWorkspace::admit_with(options(&root))
        .await
        .expect("admissible again")
        .expect("admitted after the first released");
    assert!(
        next.generation() > generation,
        "a new admission is a new generation"
    );
    next.shutdown().await.expect("ordered shutdown");
}
