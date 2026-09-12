use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::tas::owner_wire::Greeting;
use crate::tas::{
    AttachError, AttachKind, AttachRefusal, CallerIdentity, NativeReader, OwnerClient,
    OwnerCommand, OwnerReply, ProjectAuthority, RoadmapCommand, TasConfig, WriterRole,
    owner_socket_path,
};
use think_and_ship::roadmap::domain::ChunkStatus;

const DEADLINE: Duration = Duration::from_secs(2);

fn config(tmp: &tempfile::TempDir) -> TasConfig {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    // The store under a tempdir is too deep for `sun_path`; the runtime
    // directory is where the endpoint then lands, and `/tmp` always fits.
    TasConfig::from_lookup(root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_runtime_dir("/tmp")
}

fn host(cfg: &TasConfig) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, "lease".into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn admit_host(cfg: &TasConfig) -> io::Result<ProjectAuthority> {
    ProjectAuthority::admit_for_project(cfg, "lease".into(), WriterRole::Host)
}

fn client(cfg: &TasConfig) -> Result<OwnerClient, AttachError> {
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

fn chunk_ids(cfg: &TasConfig) -> Vec<String> {
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
        "build": env!("CARGO_PKG_VERSION"),
        "identity": { "kind": "workflow", "project_id": project_id, "role": role },
        "attach": attach,
        "caller": {"pid": std::process::id(), "surface": "test"},
    })
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
    let served = crate::tas::OWNER_PROTOCOL_VERSION;

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
    // Same protocol, another build of the binary: refused before identity.
    let mut skewed = hello("lease", "host", "commands", served);
    skewed["build"] = serde_json::json!("0.0.0-other");
    assert_eq!(
        refused(raw_greeting(path, skewed)),
        AttachRefusal::Build {
            served: env!("CARGO_PKG_VERSION").into(),
            offered: "0.0.0-other".into()
        }
    );
    assert_eq!(
        refused(raw_greeting(
            path,
            hello("lease", "service", "commands", served)
        )),
        AttachRefusal::Role {
            expected: WriterRole::Host,
            offered: WriterRole::Service
        }
    );
    assert_eq!(
        refused(raw_greeting(path, hello("lease", "host", "mcp", served))),
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
    let acceptor: McpAcceptor = Arc::new(|stream: tokio::net::UnixStream, cancel| {
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
fn generations_grow_with_each_admission_of_a_role_and_roles_count_separately() {
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
    let service = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Service)
        .expect("service");
    assert_eq!(service.generation().get(), 1, "its own counter");
    // The lock inode is the same file the counter lives in.
    let lock = cfg.data_dir.join("roadmap/sessions/lease.host-owner.lock");
    let text = std::fs::read_to_string(lock).expect("lock text");
    assert_eq!(text.trim().parse::<u64>().expect("counter"), g1.get() + 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_mcp_only_owner_refuses_typed_commands_at_the_handshake() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let authority = Arc::new(admit_host(&cfg).expect("admitted"));
    // An acceptor that holds the session open until the client leaves.
    let acceptor: McpAcceptor = Arc::new(|stream: tokio::net::UnixStream, cancel| {
        Box::pin(async move {
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = async move {
            let mut sink = tokio::io::sink();
            let _ = tokio::io::copy(&mut tokio::io::BufReader::new(stream), &mut sink).await;
                } => {},
            }
        })
    });
    let endpoint = OwnerEndpoint::serve_mcp(Arc::clone(&authority), acceptor)
        .await
        .expect("mcp endpoint");
    assert_eq!(endpoint.generation(), authority.generation());
    let refused = client(&cfg).expect_err("no writer to command");
    assert!(
        matches!(
            refused,
            AttachError::Refused(AttachRefusal::Unsupported(AttachKind::Commands))
        ),
        "{refused}"
    );
    OwnerClient::attach_mcp(
        &cfg,
        "lease".into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        DEADLINE,
    )
    .expect("mcp is what it offers");
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
