use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::native::owner_wire::Greeting;
use crate::native::{
    AttachError, AttachKind, AttachRefusal, BuildIdentity, CallerIdentity, NativeConfig,
    NativeReader, OWNER_PROTOCOL_VERSION, OwnerClient, OwnerCommand, OwnerIdentity, OwnerLiveness,
    OwnerProbe, OwnerReply, ProjectAuthority, RoadmapCommand, WriterRole, owner_socket_path,
};
use magistr_workflow::roadmap::domain::ChunkStatus;

const DEADLINE: Duration = Duration::from_secs(2);

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    // The store under a tempdir is too deep for `sun_path`; the runtime
    // directory is where the endpoint then lands, and `/tmp` always fits.
    NativeConfig::from_lookup(root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_runtime_dir("/tmp")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_attachment_uses_the_same_captured_identity_as_its_owner() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    magistr_workflow::infra::write_project_file(&root, "pinned-project", None)
        .expect("project pin");
    let cfg = NativeConfig::from_lookup(root, |key| {
        (key == "THINK_AND_SHIP_PROJECT_NAME").then(|| "Captured Project".into())
    })
    .with_data_dir(tmp.path().join("data"))
    .with_runtime_dir("/tmp");
    let owner = ProjectAuthority::admit(&cfg, WriterRole::Host)
        .expect("owner admitted")
        .open();
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");

    let mut attached = OwnerClient::attach_project(
        &cfg,
        WriterRole::Host,
        CallerIdentity::this_process("captured-config-test"),
    )
    .expect("attach to the captured project");
    assert_eq!(owner.project_id(), "captured-project");
    assert_eq!(attached.path(), endpoint.path());
    assert_eq!(attached.generation(), endpoint.generation());
    attached
        .command(add_chunk("captured-attachment"))
        .expect("owner command");
    assert!(
        NativeReader::open(&cfg)
            .roadmap_chunk("captured-attachment")
            .is_some()
    );
}

fn host(cfg: &NativeConfig) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, "lease".into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn admit_host(cfg: &NativeConfig) -> io::Result<ProjectAuthority> {
    ProjectAuthority::admit_for_project(cfg, "lease".into(), WriterRole::Host)
}

fn client(cfg: &NativeConfig) -> Result<OwnerClient, AttachError> {
    OwnerClient::attach_with(
        cfg,
        "lease".into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        DEADLINE,
    )
}

fn add_chunk(id: &str) -> OwnerCommand {
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

fn chunk_ids(cfg: &NativeConfig) -> Vec<String> {
    NativeReader::open_for_project(cfg, "lease".into())
        .roadmap_chunks()
        .into_iter()
        .map(|c| c.id)
        .collect()
}

/// Send a raw handshake line to `path` and read the owner's answer.
fn raw_greeting(path: &std::path::Path, hello: serde_json::Value) -> Greeting {
    let mut stream = UnixStream::connect(path).expect("connect");
    stream.set_read_timeout(Some(DEADLINE)).expect("timeout");
    writeln!(stream, "{hello}").expect("hello");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .expect("greeting");
    serde_json::from_str(line.trim_end()).expect("a greeting")
}

fn hello(project_id: &str, role: &str, attach: &str, protocol: u32) -> serde_json::Value {
    serde_json::json!({
        "protocol": protocol,
        "build": BuildIdentity::this_build(),
        "identity": { "kind": "workflow", "project_id": project_id, "role": role },
        "attach": attach,
        "caller": {"pid": std::process::id(), "surface": "test"},
    })
}

/// The `Hello` of this same project at another build: `build` replaced by
/// `other`, everything else as this process would send it.
fn hello_from(other: &BuildIdentity) -> serde_json::Value {
    let mut hello = hello("lease", "host", "commands", OWNER_PROTOCOL_VERSION);
    hello["build"] = serde_json::to_value(other).expect("identity");
    hello
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_occupied_host_is_reached_through_its_endpoint_and_takes_no_second_lease() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    assert_eq!(
        admit_host(&cfg).expect_err("occupied").kind(),
        io::ErrorKind::WouldBlock
    );

    let mut attached = client(&cfg).expect("attached");
    assert_eq!(attached.generation(), endpoint.generation());
    assert_eq!(attached.owner_pid(), std::process::id());
    assert_eq!(attached.path(), endpoint.path());
    let reply = attached.command(add_chunk("via-client")).expect("command");
    let OwnerReply::Chunk(chunk) = reply else {
        panic!("a chunk comes back, got {reply:?}");
    };
    assert_eq!(chunk.id, "via-client");
    assert_eq!(chunk_ids(&cfg), vec!["via-client".to_owned()]);
    // The owner's own verbs refuse the same way over the wire.
    let refused = attached
        .command(add_chunk("via-client"))
        .expect_err("duplicate id");
    assert!(matches!(refused, AttachError::Engine(_)), "{refused}");
    assert!(attached.is_attached(), "an engine refusal keeps the link");
    // The client holds no lease of its own.
    assert_eq!(
        admit_host(&cfg).expect_err("still occupied").kind(),
        io::ErrorKind::WouldBlock
    );
    drop(attached);
    drop(endpoint);
    drop(owner);
    admit_host(&cfg).expect("released with the writer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_generation_is_refused_until_the_client_reconnects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let first = host(&cfg);
    let first_endpoint = OwnerEndpoint::serve(first.clone()).await.expect("endpoint");
    let g1 = first_endpoint.generation();
    let mut attached = client(&cfg).expect("attached");
    assert_eq!(attached.generation(), g1);

    // The project changes hands.
    drop(first_endpoint);
    drop(first);
    let second = admit_host(&cfg).expect("released").open();
    let second_endpoint = OwnerEndpoint::serve(second.clone())
        .await
        .expect("endpoint");
    let g2 = second_endpoint.generation();
    assert!(g2 > g1, "{g2} > {g1}");

    let gone = attached.command(add_chunk("late")).expect_err("hung up");
    assert!(matches!(gone, AttachError::OwnerGone), "{gone}");
    assert!(!attached.is_attached());
    assert!(matches!(
        attached.command(add_chunk("late")).expect_err("detached"),
        AttachError::Detached
    ));

    assert_eq!(attached.reconnect().expect("reconnect"), g2);
    let stale = attached
        .command_as(g1, add_chunk("stale"))
        .expect_err("stale generation");
    assert!(
        matches!(stale, AttachError::StaleGeneration { current } if current == g2),
        "{stale}"
    );
    assert!(
        attached.is_attached(),
        "a refusal is an answer, not a hang-up"
    );
    attached
        .command(add_chunk("fresh"))
        .expect("current generation");
    assert_eq!(chunk_ids(&cfg), vec!["fresh".to_owned()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn detaching_one_client_cancels_nothing_for_the_owner_or_another() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let _endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let mut a = client(&cfg).expect("a");
    let mut b = client(&cfg).expect("b");
    a.command(add_chunk("from-a")).expect("a writes");
    a.detach();
    assert!(!a.is_attached());
    b.command(add_chunk("from-b")).expect("b still writes");
    owner
        .roadmap_start_chunk("from-a")
        .expect("the owner still writes");
    a.reconnect().expect("a comes back");
    a.command(add_chunk("from-a-again"))
        .expect("a writes again");
    let mut ids = chunk_ids(&cfg);
    ids.sort();
    assert_eq!(ids, vec!["from-a", "from-a-again", "from-b"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_silent_owner_trips_the_command_deadline_and_the_client_can_reconnect() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let path = owner_socket_path(&cfg, "lease", WriterRole::Host);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    let _ = std::fs::remove_file(&path);
    // An owner that welcomes and then never answers a command.
    let listener = tokio::net::UnixListener::bind(&path).expect("fake owner");
    let silent = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
                let (read, mut write) = stream.into_split();
                let mut lines = tokio::io::BufReader::new(read).lines();
                let _hello = lines.next_line().await;
                let welcome = serde_json::json!({
                    "greeting": "welcome", "generation": 7, "owner_pid": 1,
                });
                let _ = write.write_all(format!("{welcome}\n").as_bytes()).await;
                while let Ok(Some(_)) = lines.next_line().await {}
            });
        }
    });
    let deadline = Duration::from_millis(150);
    let mut attached = OwnerClient::attach_with(
        &cfg,
        "lease".into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        deadline,
    )
    .expect("welcomed");
    assert_eq!(attached.generation().get(), 7);
    let started = std::time::Instant::now();
    let timed_out = attached.command(add_chunk("never")).expect_err("deadline");
    assert!(
        matches!(timed_out, AttachError::Deadline(limit) if limit == deadline),
        "{timed_out}"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!attached.is_attached(), "a deadline spends the connection");
    assert_eq!(attached.reconnect().expect("reconnect").get(), 7);
    silent.abort();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_handshake_is_validated_before_anything_is_served() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let path = endpoint.path();
    let served = crate::native::OWNER_PROTOCOL_VERSION;

    let refused = |greeting: Greeting| match greeting {
        Greeting::Refused(refusal) => refusal,
        Greeting::Welcome { .. } => panic!("must be refused"),
    };
    assert_eq!(
        refused(raw_greeting(path, hello("lease", "host", "commands", 99))),
        AttachRefusal::Protocol {
            served,
            offered: 99
        }
    );
    assert_eq!(
        refused(raw_greeting(
            path,
            hello("other", "host", "commands", served)
        )),
        AttachRefusal::Project {
            expected: "lease".into(),
            offered: "other".into()
        }
    );
    // Same protocol, another package version: refused before identity.
    let this = BuildIdentity::this_build();
    let other_version = BuildIdentity::new(
        "0.0.0-other",
        this.contract(),
        this.profile(),
        this.target(),
    );
    assert_eq!(
        refused(raw_greeting(path, hello_from(&other_version))),
        AttachRefusal::Build {
            served: Box::new(this),
            offered: Box::new(other_version),
        }
    );
    let mut service_hello = hello("lease", "service", "commands", served);
    service_hello["context"] = serde_json::to_value(cfg.execution_context()).expect("context");
    assert!(matches!(
        raw_greeting(path, service_hello),
        Greeting::Welcome { .. }
    ));
    let mut unsupported = hello("lease", "host", "mcp", served);
    unsupported["context"] = serde_json::to_value(cfg.execution_context()).expect("context");
    assert_eq!(
        refused(raw_greeting(path, unsupported)),
        AttachRefusal::Unsupported(AttachKind::Mcp)
    );
    // A different project id derives a different socket: nothing answers.
    let elsewhere = OwnerClient::attach_with(
        &cfg,
        "other".into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        DEADLINE,
    )
    .expect_err("no owner for another project");
    assert!(
        matches!(elsewhere, AttachError::Unavailable(_)),
        "{elsewhere}"
    );
    // And the typed client still gets through.
    let welcomed = client(&cfg).expect("welcomed");
    assert!(welcomed.is_attached());
    assert!(chunk_ids(&cfg).is_empty(), "refusals wrote nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn equal_package_versions_from_other_builds_are_refused_with_both_identities() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let path = endpoint.path();
    let this = BuildIdentity::this_build();

    // Another checkout of the same package version: the contract digest
    // differs, nothing else does.
    let other_checkout = BuildIdentity::new(
        this.package(),
        "ffffffffffffffff",
        this.profile(),
        this.target(),
    );
    // The other profile of this very tree.
    let other_profile = BuildIdentity::new(
        this.package(),
        this.contract(),
        if this.profile() == "debug" {
            "release"
        } else {
            "debug"
        },
        this.target(),
    );
    for offered in [other_checkout, other_profile] {
        assert_eq!(offered.package(), this.package(), "same package version");
        let Greeting::Refused(refusal) = raw_greeting(path, hello_from(&offered)) else {
            panic!("another build must be refused: {offered}");
        };
        assert_eq!(
            refusal,
            AttachRefusal::Build {
                served: Box::new(this.clone()),
                offered: Box::new(offered.clone()),
            }
        );
        let text = refusal.to_string();
        assert!(text.contains(&this.to_string()), "{text}");
        assert!(text.contains(&offered.to_string()), "{text}");
    }

    // A copy of this build — the same identity from another process — is
    // the same build and attaches.
    let copied = BuildIdentity::new(
        this.package(),
        this.contract(),
        this.profile(),
        this.target(),
    );
    assert!(
        matches!(
            raw_greeting(path, {
                let mut hello = hello_from(&copied);
                hello["context"] = serde_json::to_value(cfg.execution_context()).expect("context");
                hello
            }),
            Greeting::Welcome { .. }
        ),
        "an equal build attaches"
    );
    assert!(chunk_ids(&cfg).is_empty(), "refusals wrote nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mismatched_protocol_is_refused_before_reading_its_payload() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let offered = OWNER_PROTOCOL_VERSION + 1;
    let mismatched = serde_json::json!({
        "protocol": offered,
        "build": BuildIdentity::this_build(),
        "identity": { "kind": "workflow", "project_id": "lease", "role": "host" },
        "attach": "commands",
        "caller": {"pid": std::process::id(), "surface": "test"},
    });
    assert_eq!(
        raw_greeting(endpoint.path(), mismatched),
        Greeting::Refused(AttachRefusal::Protocol {
            served: OWNER_PROTOCOL_VERSION,
            offered,
        })
    );
    // A line that is not a `Hello` of any version gets no greeting at all.
    let mut stream = UnixStream::connect(endpoint.path()).expect("connect");
    stream.set_read_timeout(Some(DEADLINE)).expect("timeout");
    writeln!(stream, "{}", serde_json::json!({"protocol": "four"})).expect("write");
    let mut line = String::new();
    let read = BufReader::new(stream).read_line(&mut line).expect("eof");
    assert_eq!(read, 0, "dropped without a greeting: {line}");
    assert!(client(&cfg).is_ok(), "the served protocol still attaches");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_shutdown_leaves_an_attached_client_without_a_writer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let mut attached = client(&cfg).expect("attached");
    attached.command(add_chunk("before")).expect("writes");
    let path = endpoint.path().to_path_buf();

    // The owner stops accepting work and releases ownership.
    drop(endpoint);
    drop(owner);
    assert!(!path.exists(), "the endpoint unlinks its socket");

    let gone = attached.command(add_chunk("after")).expect_err("hung up");
    assert!(matches!(gone, AttachError::OwnerGone), "{gone}");
    let nobody = attached.reconnect().expect_err("nothing to attach to");
    assert!(matches!(nobody, AttachError::Unavailable(_)), "{nobody}");
    assert!(matches!(
        attached
            .command(add_chunk("after"))
            .expect_err("still detached"),
        AttachError::Detached
    ));
    // The client opened no writer of its own: the role is free.
    let fresh = admit_host(&cfg).expect("free");
    assert_eq!(chunk_ids(&cfg), vec!["before".to_owned()]);
    drop(fresh);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_mcp_attachment_hands_the_socket_to_the_acceptor_after_the_welcome() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let acceptor: McpAcceptor = Arc::new(|stream: tokio::net::UnixStream, cancel, _context| {
        Box::pin(async move {
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = async move {
            let (read, mut write) = stream.into_split();
            let mut lines = tokio::io::BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if write
                    .write_all(format!("echo:{line}\n").as_bytes())
                    .await
                    .is_err()
                {
                    return;
                }
            }
                } => {},
            }
        })
    });
    let endpoint = OwnerEndpoint::serve_with(owner.clone(), Some(acceptor))
        .await
        .expect("endpoint");

    let (generation, mut stream) = OwnerClient::attach_mcp(
        &cfg,
        "lease".into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        DEADLINE,
    )
    .expect("mcp attached");
    assert_eq!(generation, endpoint.generation());
    writeln!(stream, "ping").expect("write");
    let mut line = String::new();
    BufReader::new(stream.try_clone().expect("clone"))
        .read_line(&mut line)
        .expect("echo");
    assert_eq!(line.trim_end(), "echo:ping");
    // Typed commands are served on the same endpoint.
    let mut typed = client(&cfg).expect("commands");
    typed.command(add_chunk("beside-mcp")).expect("writes");
    assert_eq!(chunk_ids(&cfg), vec!["beside-mcp".to_owned()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_endpoint_is_refused_and_a_stale_socket_is_reclaimed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let twice = OwnerEndpoint::serve(owner.clone())
        .await
        .expect_err("one endpoint per owner");
    assert_eq!(twice.kind(), io::ErrorKind::AddrInUse, "{twice}");
    let path = endpoint.path().to_path_buf();
    drop(endpoint);
    // A socket file nobody listens on — a crashed owner's leftover.
    drop(std::os::unix::net::UnixListener::bind(&path).expect("stale"));
    assert!(path.exists());
    let again = OwnerEndpoint::serve(owner.clone())
        .await
        .expect("reclaimed");
    assert_eq!(again.path(), path);
    client(&cfg).expect("attaches to the reclaimed endpoint");
}

#[test]
fn generations_grow_across_host_and_service_admissions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let first = admit_host(&cfg).expect("first");
    let g1 = first.generation();
    assert_eq!(
        first.owner_socket_path(),
        owner_socket_path(&cfg, "lease", WriterRole::Host)
    );
    drop(first);
    let second = admit_host(&cfg).expect("second");
    assert_eq!(second.generation().get(), g1.get() + 1);
    drop(second);
    let service = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Service)
        .expect("service after host");
    assert_eq!(
        service.generation().get(),
        g1.get() + 2,
        "one project counter"
    );
    // The lock inode is the same file the counter lives in.
    let lock = cfg.workflow_root().join(".magistr/workflow-owner.lock");
    let text = std::fs::read_to_string(lock).expect("lock text");
    assert_eq!(text.trim().parse::<u64>().expect("counter"), g1.get() + 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_regular_file_at_the_owner_endpoint_is_preserved() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let path = owner_socket_path(&cfg, "lease", WriterRole::Host);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
    std::fs::write(&path, b"preserve this file").expect("occupant");

    let result = OwnerEndpoint::serve(owner).await;
    assert!(
        result.is_err(),
        "an occupied regular file must never become a socket"
    );
    assert_eq!(
        std::fs::read(&path).expect("preserved"),
        b"preserve this file"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_probe_is_answered_with_a_report_and_refused_like_any_other_build() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner.clone()).await.expect("endpoint");
    let path = endpoint.path();
    let this = BuildIdentity::this_build();

    // A probe from another build learns the served build and nothing else.
    let other = BuildIdentity::new(
        "0.0.0-other",
        this.contract(),
        this.profile(),
        this.target(),
    );
    let mut hello = hello("lease", "host", "probe", OWNER_PROTOCOL_VERSION);
    hello["build"] = serde_json::to_value(&other).expect("identity");
    assert_eq!(
        raw_greeting(path, hello),
        Greeting::Refused(AttachRefusal::Build {
            served: Box::new(this.clone()),
            offered: Box::new(other),
        })
    );

    // A typed probe of this build: the welcome, then the report, then EOF.
    let mut attached = client(&cfg).expect("a commands session to be counted");
    let probed = OwnerProbe::at(
        path,
        OwnerIdentity::Workflow {
            project_id: "lease".into(),
            role: WriterRole::Host,
        },
        CallerIdentity::this_process("probe-test"),
        DEADLINE,
    );
    let OwnerLiveness::Live(report) = probed.liveness else {
        panic!("live: {:?}", probed.liveness);
    };
    assert_eq!(report.generation, endpoint.generation());
    assert_eq!(report.owner_pid, std::process::id());
    assert_eq!(report.build, this);
    assert_eq!(report.attached(AttachKind::Commands), 1);
    assert_eq!(report.connections[0].caller.surface, "test");
    attached.detach();
    assert!(chunk_ids(&cfg).is_empty(), "a probe writes nothing");
}
