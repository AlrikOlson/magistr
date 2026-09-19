use std::sync::mpsc;
use std::time::Duration;

use super::super::{IndexOptions, WorkspaceIndex, watch::IndexWatcher, watch::WatchMode};
use crate::contracts::Freshness;
use crate::worker::{WorkerOptions, WorkerSupervisor};
use std::sync::Arc;

/// Starting the operating system's file watcher can take half a second, and
/// seconds when many start at once. It is its own step: the cache opens and
/// the first scan finishes without it, a stop never waits for it, and once it
/// is installed the tree is scanned again so nothing that changed meanwhile
/// is missed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_index_opens_scans_and_stops_without_waiting_for_the_watcher() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let worker = Arc::new(
        WorkerSupervisor::new(std::env::current_exe().unwrap(), WorkerOptions::default()).unwrap(),
    );
    let (release, held) = mpsc::channel::<()>();
    let (started, starting) = mpsc::channel::<()>();
    let mut index = WorkspaceIndex::start_with(
        root.clone(),
        dir.path().join("state"),
        worker,
        IndexOptions::default(),
        Box::new(move |root, _mode, _network, interval| {
            started.send(()).unwrap();
            // Held until the test lets go: the slow operating system call.
            let _ = held.recv();
            IndexWatcher::start(root, WatchMode::Poll, false, interval)
        }),
    )
    .unwrap();
    starting
        .recv_timeout(Duration::from_secs(10))
        .expect("the watcher start was asked for");
    // The first scan completes while the watcher is still starting. Without a
    // watcher the index never claims to be warm.
    let scanned = async {
        loop {
            let status = index.status();
            if status.build_started_at_ms.is_some() && !status.building {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    let status = tokio::time::timeout(Duration::from_secs(10), scanned)
        .await
        .expect("the first scan does not wait for the watcher");
    assert_ne!(status.freshness, Freshness::Warm);
    // Nor does a stop: it returns while the start is still held.
    tokio::time::timeout(Duration::from_secs(10), index.shutdown())
        .await
        .expect("a stop does not wait for the watcher")
        .unwrap();
    assert_eq!(index.status().freshness, Freshness::Cold);
    // The late watcher is torn down by its own step once the call returns.
    release.send(()).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_installed_watcher_makes_the_index_warm_after_a_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let worker = Arc::new(
        WorkerSupervisor::new(std::env::current_exe().unwrap(), WorkerOptions::default()).unwrap(),
    );
    let mut index = WorkspaceIndex::start_with(
        root,
        dir.path().join("state"),
        worker,
        IndexOptions::default(),
        Box::new(|root, _mode, _network, interval| {
            IndexWatcher::start(root, WatchMode::Poll, false, interval)
        }),
    )
    .unwrap();
    let warm = async {
        while index.status().freshness != Freshness::Warm {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(20), warm)
        .await
        .expect("warm once the watcher is installed and the tree rescanned");
    index.shutdown().await.unwrap();
}
