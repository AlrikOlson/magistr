use std::{path::Path, sync::Arc, time::Duration};

use magistr_engine::native::{CallerIdentity, McpAcceptor, OwnerClient, OwnerEndpoint};
use magistr_workspace::{diagnostics::DoctorSeverity, service::ServerLock};

use super::*;

const WAIT: Duration = Duration::from_secs(5);

#[test]
fn a_different_build_limits_diagnostics_without_reporting_a_broken_connection() {
    let served = BuildIdentity::new(
        "0.1.0",
        "1111111111111111",
        "release",
        "aarch64-apple-darwin",
    );
    let offered = BuildIdentity::new("0.1.0", "2222222222222222", "debug", "aarch64-apple-darwin");
    let liveness = OwnerLiveness::Refused(AttachRefusal::Build {
        served: Box::new(served),
        offered: Box::new(offered),
    });
    let mut findings = Vec::new();
    probe_findings(&liveness, "fixture", Some(1), &mut findings);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, DoctorSeverity::Info);
    assert!(
        findings[0].remediation.is_none(),
        "do not tell users to end working sessions"
    );
    assert!(
        findings[0]
            .symptom
            .contains("does not determine whether MCP tools can connect")
    );
}

fn acceptor() -> McpAcceptor {
    Arc::new(|stream: tokio::net::UnixStream, cancel, _context| {
        Box::pin(async move {
            let (mut read, mut write) = stream.into_split();
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = tokio::io::copy(&mut read, &mut write) => {},
            }
        })
    })
}

async fn observe(root: &Path) -> DoctorSection {
    let root = root.to_path_buf();
    let state = root.join(".workbench");
    tokio::task::spawn_blocking(move || owner_section(&root, &state, PROBE_DEADLINE))
        .await
        .expect("section task")
}

fn owner_finding(section: &DoctorSection) -> &DoctorFinding {
    section
        .findings
        .iter()
        .find(|finding| finding.store == "owner")
        .expect("owner finding")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_owner_is_reported_with_its_generation_build_and_rising_falling_sessions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical");
    let state = root.join(".workbench");
    std::fs::create_dir_all(&state).expect("state");
    let lock = ServerLock::acquire(state.join(ServerLock::FILE_NAME)).expect("owner");
    let generation = lock.generation().get();
    let endpoint = OwnerEndpoint::serve_workspace(&root, generation, acceptor())
        .await
        .expect("endpoint");

    let section = observe(&root).await;
    assert_eq!(section.id, DoctorSectionId::Owner);
    let finding = owner_finding(&section);
    assert_eq!(
        finding.severity,
        DoctorSeverity::Healthy,
        "{}",
        finding.symptom
    );
    assert!(
        finding.symptom.contains(&format!(
            "live owner generation {generation}, pid {}",
            std::process::id()
        )),
        "{}",
        finding.symptom
    );
    assert!(
        finding.symptom.contains("0 live connection(s)"),
        "{}",
        finding.symptom
    );
    assert_eq!(section.detail["probe"]["liveness"], "live");
    assert_eq!(section.detail["probe"]["attached"]["total"], 0);
    assert_eq!(section.detail["durableGeneration"], generation);
    assert_eq!(section.detail["lockRecord"]["kind"], "native");
    assert!(
        !section
            .findings
            .iter()
            .any(|f| f.severity == DoctorSeverity::Unhealthy),
        "{:?}",
        section.findings
    );

    let (_, attached) = OwnerClient::attach_workspace(
        &root,
        CallerIdentity::this_process("bridge-under-test"),
        WAIT,
    )
    .await
    .expect("attached");
    let section = observe(&root).await;
    assert!(
        owner_finding(&section)
            .symptom
            .contains("1 live connection(s): 1 mcp, 0 commands"),
        "{}",
        owner_finding(&section).symptom
    );
    assert_eq!(
        section.detail["probe"]["connections"][0]["caller"]["surface"],
        "bridge-under-test"
    );
    drop(attached);
    tokio::time::timeout(WAIT, async {
        loop {
            let section = observe(&root).await;
            if section.detail["probe"]["attached"]["total"] == 0 {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the count falls on EOF");
    endpoint.shutdown().await.expect("shutdown");
    drop(lock);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_counter_that_disagrees_with_the_live_owner_is_a_durable_finding() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical");
    let state = root.join(".workbench");
    std::fs::create_dir_all(&state).expect("state");
    let lock = ServerLock::acquire(state.join(ServerLock::FILE_NAME)).expect("owner");
    let endpoint = OwnerEndpoint::serve_workspace(&root, 99, acceptor())
        .await
        .expect("endpoint");
    let section = observe(&root).await;
    let disagreement = section
        .findings
        .iter()
        .find(|f| f.store == "server.generation")
        .expect("counter finding");
    assert_eq!(disagreement.severity, DoctorSeverity::Unhealthy);
    assert!(
        disagreement
            .symptom
            .contains("says 1 but the live owner was admitted under 99"),
        "{}",
        disagreement.symptom
    );
    endpoint.shutdown().await.expect("shutdown");
    drop(lock);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn records_never_certify_liveness_and_legacy_markers_name_the_migration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical");
    // Never admitted: no counter, no record, no endpoint.
    let section = observe(&root).await;
    let finding = owner_finding(&section);
    assert_eq!(finding.severity, DoctorSeverity::Info);
    assert!(
        finding
            .symptom
            .contains("no owner reachable: no endpoint at"),
        "{}",
        finding.symptom
    );
    assert!(
        finding.symptom.contains("no admission is recorded"),
        "{}",
        finding.symptom
    );
    assert_eq!(section.detail["lockRecord"]["kind"], "absent");
    assert!(
        !root.join(".workbench").exists(),
        "the probe creates nothing"
    );

    // Admitted once, then released: the counter and marker remain, no owner.
    let state = root.join(".workbench");
    std::fs::create_dir_all(&state).expect("state");
    drop(ServerLock::acquire(state.join(ServerLock::FILE_NAME)).expect("owner"));
    let section = observe(&root).await;
    let finding = owner_finding(&section);
    assert_eq!(finding.severity, DoctorSeverity::Info);
    assert!(
        finding
            .symptom
            .contains("generation 1 as the last admission, which is not a liveness proof"),
        "{}",
        finding.symptom
    );
    assert_eq!(section.detail["probe"]["liveness"], "absent");

    // A stale socket file nobody listens on is named as such.
    let socket = magistr_engine::native::workspace_socket_path(&root).expect("endpoint path");
    std::fs::create_dir_all(socket.parent().expect("parent")).expect("run dir");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("bind"));
    let section = observe(&root).await;
    let finding = owner_finding(&section);
    assert_eq!(finding.severity, DoctorSeverity::Info);
    assert!(
        finding.symptom.contains("a stale socket certifies nothing"),
        "{}",
        finding.symptom
    );
    assert_eq!(section.detail["probe"]["liveness"], "unanswered");
    std::fs::remove_file(&socket).expect("unlink");

    // A legacy record: unhealthy, with the migration named and the pid
    // reported as a hint.
    std::fs::write(
        state.join(ServerLock::FILE_NAME),
        format!(r#"{{"pid":{},"host":"elsewhere"}}"#, std::process::id()),
    )
    .expect("legacy");
    let section = observe(&root).await;
    let lock = section
        .findings
        .iter()
        .find(|f| f.store == "server.lock")
        .expect("lock finding");
    assert_eq!(lock.severity, DoctorSeverity::Unhealthy);
    assert!(
        lock.symptom.contains("legacy server record"),
        "{}",
        lock.symptom
    );
    assert!(lock.symptom.contains("not judged here"), "{}", lock.symptom);
    assert!(
        lock.remediation
            .as_deref()
            .is_some_and(|r| r.contains("magistr workspace migrate")),
        "{:?}",
        lock.remediation
    );
    assert_eq!(
        section.detail["lockRecord"]["pidAlive"],
        serde_json::Value::Null
    );
}
