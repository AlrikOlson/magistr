use super::*;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{Notify, Semaphore};

use crate::roadmap::application::ChunkDraft;
use crate::roadmap::domain::ChunkStatus;
use crate::roadmap::engine::RoadmapEngine;
use crate::tracker::{TargetInfo, TrackerCapabilities, TrackerError, UpsertOutcome, WorkItem};

struct ProbePort {
    entered: Notify,
    release: Semaphore,
    result: Result<TargetInfo, TrackerError>,
    creations: Mutex<Vec<String>>,
}

impl ProbePort {
    fn missing() -> Self {
        Self {
            entered: Notify::new(),
            release: Semaphore::new(1),
            result: Err(TrackerError::NotFound("acme/widgets".into())),
            creations: Mutex::new(vec![]),
        }
    }
}

fn target(name: &str) -> TargetInfo {
    TargetInfo {
        key: "acme/widgets".into(),
        name: name.into(),
        detail: "Test destination".into(),
    }
}

#[async_trait]
impl TrackerPort for ProbePort {
    fn provider(&self) -> &str {
        "github"
    }

    fn capabilities(&self) -> TrackerCapabilities {
        TrackerCapabilities::full()
    }

    async fn upsert_item(&self, _: &WorkItem) -> Result<UpsertOutcome, TrackerError> {
        panic!("setup must not project items")
    }

    async fn fetch_since(&self, _: &str) -> Result<Vec<WorkItem>, TrackerError> {
        panic!("setup must not fetch items")
    }

    async fn probe_target(&self) -> Result<TargetInfo, TrackerError> {
        self.entered.notify_one();
        self.release.acquire().await.unwrap().forget();
        self.result.clone()
    }

    async fn create_target(&self, name: &str) -> Result<TargetInfo, TrackerError> {
        self.creations.lock().unwrap().push(name.into());
        Ok(target(name))
    }
}

/// A roadmap over a ship family with no evidence capability: nothing here
/// completes a chunk.
fn application() -> RoadmapUseCases {
    RoadmapUseCases::new(
        Arc::new(Mutex::new(RoadmapEngine::new("project".into()))),
        crate::ship::ShipUseCases::new(Arc::new(Mutex::new(crate::ship::engine::ShipEngine::new(
            "project".into(),
        )))),
    )
}

fn draft(id: &str) -> ChunkDraft {
    ChunkDraft {
        id: id.into(),
        title: id.into(),
        name: String::new(),
        status: ChunkStatus::Pending,
        priority: 10,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        shared: false,
        group: None,
        content: None,
    }
}

fn request() -> SetupRequest {
    SetupRequest {
        provider: "github".into(),
        into: "acme/widgets".into(),
        name: Some("Widgets".into()),
        band: None,
        initiative: None,
        push: false,
        // No host configuration discovery or writes in these isolated tests.
        push_secs: 0,
        yes: false,
        dry_run: false,
    }
}

#[tokio::test]
async fn awaiting_probe_leaves_native_reads_and_mutations_available() {
    let app = application();
    app.add_chunk(draft("existing")).unwrap();
    let port = Arc::new(ProbePort {
        release: Semaphore::new(0),
        result: Ok(target("Widgets")),
        ..ProbePort::missing()
    });
    let directory = tempfile::tempdir().unwrap();
    let task = {
        let app = app.clone();
        let port = Arc::clone(&port);
        let path = directory.path().to_owned();
        tokio::spawn(async move {
            app.setup_tracker(port.as_ref(), &request(), false, &path)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(10), port.entered.notified())
        .await
        .expect("setup reaches the suspended remote probe");

    // A retained synchronous guard fails immediately rather than hanging the
    // runtime while the peer client below attempts its native operations.
    assert!(app.engine().try_lock().is_ok(), "probe retains native lock");
    assert_eq!(app.next().unwrap().unwrap().id, "existing");
    app.add_chunk(draft("arrived-during-probe")).unwrap();
    assert_eq!(app.status().unwrap()["counts"]["total"], 2);
    assert!(
        !task.is_finished(),
        "probe must still await its explicit release"
    );

    port.release.add_permits(1);
    let report = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(report.phase, TargetPhase::Present);
    assert_eq!(report.outcome.included.len(), 2);
    assert!(
        report
            .outcome
            .included
            .contains(&"arrived-during-probe".into())
    );
    let config = crate::tracker::config::load(directory.path(), "project");
    assert_eq!(app.tracker_status(&config).unwrap().included, 2);
    assert!(port.creations.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_destination_without_consent_writes_neither_native_nor_local_state() {
    let app = application();
    app.add_chunk(draft("existing")).unwrap();
    let before = app.engine().lock().unwrap().roadmap().clone();
    let directory = tempfile::tempdir().unwrap();
    let port = ProbePort::missing();
    let mut req = request();
    // The CLI's prompt flag is deliberately not consent at this application
    // boundary: only the explicit create_missing argument permits creation.
    req.yes = true;
    let failure = app
        .setup_tracker(&port, &req, false, directory.path())
        .await
        .unwrap_err();
    assert_eq!(failure.kind, "target_missing");
    assert!(failure.message.contains("Nothing was written"));
    assert!(port.creations.lock().unwrap().is_empty());
    assert_eq!(*app.engine().lock().unwrap().roadmap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn explicit_creation_consent_is_honoured_except_during_dry_run() {
    for dry_run in [true, false] {
        let app = application();
        app.add_chunk(draft("existing")).unwrap();
        let before = app.engine().lock().unwrap().roadmap().clone();
        let directory = tempfile::tempdir().unwrap();
        let port = ProbePort::missing();
        let mut req = request();
        req.dry_run = dry_run;
        let report = app
            .setup_tracker(&port, &req, true, directory.path())
            .await
            .unwrap();
        assert_eq!(report.outcome.included, vec!["existing"]);
        assert_eq!(report.outcome.mirroring_enabled, !dry_run);
        if dry_run {
            assert_eq!(report.phase, TargetPhase::MissingAndNotCreated);
            assert!(port.creations.lock().unwrap().is_empty());
            assert_eq!(*app.engine().lock().unwrap().roadmap(), before);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        } else {
            assert_eq!(report.phase, TargetPhase::Created);
            assert_eq!(*port.creations.lock().unwrap(), vec!["Widgets"]);
            let config = crate::tracker::config::load(directory.path(), "project");
            assert!(config.enabled);
            assert_eq!(config.target.as_deref(), Some("acme/widgets"));
            assert_eq!(app.tracker_status(&config).unwrap().included, 1);
            // The live engine inherits the opt-in without a restart.
            app.add_chunk(draft("later")).unwrap();
            assert_eq!(app.tracker_status(&config).unwrap().included, 2);
        }
    }
}
