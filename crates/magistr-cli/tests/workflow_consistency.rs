//! The workflow consistency model across adapters, as
//! `docs/architecture/clients.md` states it: a Host writer serving its
//! owner endpoint, a Service writer, a client attached to the Host and an
//! unadmitted reader, all in one process on one store. The families
//! differ — roadmap and signal merge on save but answer cached reads until
//! a reload; ship refreshes the persisted cycle before every use case —
//! and duplicate ownership of a workflow role or a workspace root is
//! refused in-process, not only across processes.

#![cfg(unix)]

use std::io;
use std::path::{Path, PathBuf};

use magistr::application::HostedWorkspace;
use magistr_engine::tas::{CallerIdentity, OwnerClient, OwnerCommand, OwnerReply, RoadmapCommand};
use magistr_engine::{
    ChunkStatus, NativeEngines, NativeReader, OwnerEndpoint, ProjectAuthority, TasConfig, TaskType,
    WriterRole,
};
use magistr_workspace::service::WorkspaceOpenOptions;

const PROJECT: &str = "consistency";

/// A store under a tempdir is too deep for `sun_path`: the endpoint lands
/// in the runtime directory, and `/tmp` always fits.
fn config(tmp: &tempfile::TempDir) -> TasConfig {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    TasConfig::from_lookup(root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_runtime_dir("/tmp")
}

fn admit(cfg: &TasConfig, role: WriterRole) -> io::Result<ProjectAuthority> {
    ProjectAuthority::admit_for_project(cfg, PROJECT.into(), role)
}

fn writer(cfg: &TasConfig, role: WriterRole) -> NativeEngines {
    admit(cfg, role).expect("admitted").open()
}

fn reader(cfg: &TasConfig) -> NativeReader {
    NativeReader::open_for_project(cfg, PROJECT.into())
}

fn attached(cfg: &TasConfig) -> OwnerClient {
    OwnerClient::attach_with(
        cfg,
        PROJECT.into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        OwnerClient::DEADLINE,
    )
    .expect("attached to the host")
}

fn add_chunk(engines: &NativeEngines, id: &str) {
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("add chunk");
}

fn add_chunk_command(id: &str) -> OwnerCommand {
    OwnerCommand::Roadmap(RoadmapCommand::AddChunk {
        id: id.into(),
        title: format!("Chunk {id}"),
        name: String::new(),
        status: ChunkStatus::Pending,
        priority: 100,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        content: None,
        group: None,
    })
}

fn chunk_ids(reader: &NativeReader) -> Vec<String> {
    let mut ids: Vec<String> = reader.roadmap_chunks().into_iter().map(|c| c.id).collect();
    ids.sort();
    ids
}

fn checks_on(reader: &NativeReader, task: &str) -> Vec<String> {
    reader
        .ship_tasks()
        .into_iter()
        .filter(|t| t.id == task)
        .flat_map(|t| t.checks)
        .map(|check| check.name)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_attached_command_is_current_on_the_owner_and_stale_on_the_other_role_until_it_reloads()
{
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let endpoint = OwnerEndpoint::serve(host.clone()).await.expect("endpoint");
    let service = writer(&cfg, WriterRole::Service);
    let observer = reader(&cfg);
    // Both roles and the observer opened on an empty store.
    assert!(service.roadmap_chunks().is_empty() && observer.roadmap_chunks().is_empty());

    // The attached client's command runs on the host's cached engines:
    // read-your-writes for the owner, and on disk at once for a reload.
    let mut client = attached(&cfg);
    let OwnerReply::Chunk(chunk) = client
        .command(add_chunk_command("via-attached"))
        .expect("command")
    else {
        panic!("a chunk comes back");
    };
    assert_eq!(chunk.id, "via-attached");
    assert!(
        host.roadmap_chunk("via-attached").is_some(),
        "the owner is current"
    );
    assert_eq!(chunk_ids(&reader(&cfg)), vec!["via-attached".to_owned()]);

    // The other role's cached roadmap does not see it — not on a query, not
    // on its own next write — and its merge-save keeps the row on disk.
    assert!(
        service.roadmap_chunk("via-attached").is_none(),
        "no cross-role read-through for the roadmap"
    );
    add_chunk(&service, "via-service");
    assert!(
        service.roadmap_chunk("via-attached").is_none(),
        "a write from stale memory does not refresh the cache either"
    );
    assert_eq!(
        chunk_ids(&reader(&cfg)),
        vec!["via-attached".to_owned(), "via-service".to_owned()],
        "the locked merge-on-save erased nothing"
    );
    // The observer opened before both writes is equally stale until it reloads.
    assert!(observer.roadmap_chunks().is_empty());
    observer.reload_roadmap();
    service.reload_roadmap();
    assert_eq!(
        chunk_ids(&observer),
        vec!["via-attached".to_owned(), "via-service".to_owned()]
    );
    assert!(service.roadmap_chunk("via-attached").is_some());
    // The host, in turn, is stale about the service's chunk until it reloads.
    assert!(host.roadmap_chunk("via-service").is_none());
    host.reload_roadmap();
    assert!(host.roadmap_chunk("via-service").is_some());

    drop(client);
    drop(endpoint);
}

#[test]
fn ship_verbs_read_through_across_roles_so_a_gate_the_service_recorded_survives_a_stale_host() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let service = writer(&cfg, WriterRole::Service);
    let observer = reader(&cfg);

    // The host opens the cycle: an objective with one task, started.
    host.ship_set_objective("cycle".into(), vec![], vec![], "chunk:c1".into())
        .expect("objective");
    host.ship_add_task("verify".into(), "verify".into(), TaskType::Test, None)
        .expect("task");
    host.ship_start_task("verify").expect("start");

    // The service — opened on the empty store — records a gate on that task
    // without any reload: the use case refreshed the persisted cycle first.
    let check = service
        .ship_record_gate(
            Some("verify"),
            "lint",
            "cargo clippy".into(),
            0,
            String::new(),
            true,
            None,
        )
        .expect("the service sees the host's task");
    assert!(check.verified && check.passed);
    assert_eq!(service.ship_active_task_id().as_deref(), Some("verify"));

    // The host's cached memory predates the check; completing the task from
    // it neither loses the check nor answers a stale task.
    let completed = host.ship_complete_task("verify", vec![]).expect("complete");
    assert_eq!(
        completed
            .checks
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["lint"],
        "the host's verb answered the refreshed task"
    );
    assert_eq!(checks_on(&reader(&cfg), "verify"), vec!["lint".to_owned()]);
    assert_eq!(host.ship_status()["tasks"]["completed"], 1);

    // Only the use cases read through. A query on another role — and on a
    // reader opened before the writes — answers the cached cycle until it
    // reloads: the service still shows the task it last refreshed to (active,
    // with its gate), the observer the empty store it opened on.
    assert_eq!(
        service.ship_status()["tasks"]["completed"],
        0,
        "a query does not refresh the other role's cache"
    );
    assert_eq!(observer.ship_status()["tasks"]["total"], 0);
    service.reload_ship();
    observer.reload_ship();
    assert_eq!(service.ship_status()["tasks"]["completed"], 1);
    assert_eq!(checks_on(&observer, "verify"), vec!["lint".to_owned()]);
    assert_eq!(observer.ship_status()["tasks"]["completed"], 1);
}

#[test]
fn a_second_writer_of_a_role_is_refused_in_process_and_the_lease_ends_with_the_last_clone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = writer(&cfg, WriterRole::Host);
    let service = writer(&cfg, WriterRole::Service);
    for role in [WriterRole::Host, WriterRole::Service] {
        let refused = admit(&cfg, role).expect_err("a second writer of the role");
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
    drop(observer);
    admit(&cfg, WriterRole::Host).expect("released with the last writer clone");
    drop(service);
    admit(&cfg, WriterRole::Service).expect("released with the last writer");
}

fn workspace_root(tmp: &tempfile::TempDir) -> PathBuf {
    let root = tmp.path().join("workspace");
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("note");
    root
}

fn options(root: &Path) -> WorkspaceOpenOptions {
    WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr")).with_network_override("0")
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
