use super::*;
use crate::{
    config::ResolveOptions,
    contracts::ErrorKind,
    query::MapDetail,
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};

#[tokio::test]
async fn cold_map_is_live_scoped_and_budgeted_without_starting_the_worker() {
    let directory = tempfile::tempdir().expect("workspace");
    std::fs::create_dir(directory.path().join("src")).expect("src");
    std::fs::write(directory.path().join("src/main.rs"), "fn main() {}\n").expect("source");
    std::fs::write(directory.path().join("README.md"), "readme").expect("readme");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(directory.path())).expect("policy");
    let worker = WorkerSupervisor::new(
        std::env::current_exe().expect("executable"),
        WorkerOptions::default(),
    )
    .expect("worker");
    let queries = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let mut input = MapInput::default();
    let response = queries.map(&input).await;
    let WorkspaceResponse::Success {
        results,
        freshness,
        notes,
        ..
    } = response
    else {
        panic!("cold map")
    };
    assert_eq!(freshness, Freshness::Cold);
    assert!(
        matches!(&results.rows()[0], MapRow::Overview { files: 2, dirs: 1, ttl_ms: 2000, index: None, entry_points, .. } if entry_points.len() == 1)
    );
    assert!(
        notes
            .expect("cold notes")
            .iter()
            .any(|note| note.contains("symbol outlines appear"))
    );
    input.path = "src".into();
    input.detail = MapDetail::Skeleton;
    let WorkspaceResponse::Success { results, .. } = queries.map(&input).await else {
        panic!("scoped map")
    };
    assert!(matches!(
        &results.rows()[0],
        MapRow::Overview {
            files: 1,
            dirs: 0,
            ..
        }
    ));
    assert_eq!(results.rows()[1].path(), "src/main.rs");
    input.path = ".".into();
    input.budget = 60;
    let WorkspaceResponse::Success {
        results, budget, ..
    } = queries.map(&input).await
    else {
        panic!("budgeted map")
    };
    assert!(budget.used <= input.budget);
    let cut = results.truncation().expect("cut");
    assert!(cut.cursor().starts_with("path:"));
    assert!(cut.dropped() > 0);
    input.budget = 1;
    assert!(
        matches!(queries.map(&input).await, WorkspaceResponse::Failure { error, .. } if error.kind == ErrorKind::BudgetTooSmall)
    );
    input.path = "../outside".into();
    assert!(
        matches!(queries.map(&input).await, WorkspaceResponse::Failure { error, .. } if error.kind == ErrorKind::PathOutsideWorkspace)
    );
}

#[test]
fn entry_points_prefer_manifest_names_and_ignore_unlisted_manifests() {
    let files = ["package.json", "Cargo.toml", "src/main.rs"].map(str::to_owned);
    let entries = entry_points(&files, |path| {
        Some(
            match path {
                "package.json" => {
                    "{\"main\":\"./src/main.rs\",\"name\":\"tool\",\"bin\":\"./bin.js\"}"
                }
                "Cargo.toml" => "[[bin]]\nname=\"cli\"\n",
                _ => panic!("unexpected manifest read"),
            }
            .into(),
        )
    });
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].kind, super::super::MapEntryKind::PackageMain);
    assert_eq!(entries[1].name.as_deref(), Some("tool"));
    assert_eq!(entries[2].path, "src/bin/cli.rs");
    assert!(entry_points(&[], |_| panic!("unlisted read")).is_empty());
}

#[test]
fn final_index_health_demotes_freshness_and_ttl_after_manifest_reads() {
    let mut status = IndexStatus {
        freshness: Freshness::Warm,
        building: false,
        ..IndexStatus::default()
    };
    assert_eq!(
        final_health(Freshness::Warm, Some(&status)),
        (Freshness::Warm, 30_000)
    );
    status.freshness = Freshness::Warming;
    assert_eq!(
        final_health(Freshness::Warm, Some(&status)),
        (Freshness::Warming, 2_000)
    );
    status.freshness = Freshness::Cold;
    assert_eq!(
        final_health(Freshness::Warm, Some(&status)),
        (Freshness::Cold, 2_000)
    );
    status.freshness = Freshness::Warm;
    status.building = true;
    assert_eq!(
        final_health(Freshness::Warm, Some(&status)),
        (Freshness::Warming, 2_000)
    );
    status.building = false;
    assert_eq!(
        final_health(Freshness::Cold, Some(&status)),
        (Freshness::Cold, 2_000)
    );
}
