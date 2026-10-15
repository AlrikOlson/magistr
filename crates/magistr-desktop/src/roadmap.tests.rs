use super::*;
use magistr_cockpit::harness::fixtures;
use magistr_workflow_contracts::roadmap::{ChunkStatus, WorkstreamReadiness};
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

#[test]
fn overview_uses_complete_cross_track_dependencies_and_keeps_history() {
    let mut dependent = fixtures::chunk("dependent");
    dependent.group = Some("Desktop".into());
    dependent.deps = vec!["foundation".into()];
    let mut foundation = fixtures::chunk("foundation");
    foundation.group = Some("Platform".into());
    foundation.status = ChunkStatus::Done;
    let ungrouped = fixtures::chunk("ungrouped");
    let records = ReadState::Current(Observed {
        value: RoadmapData {
            chunks: vec![dependent, foundation, ungrouped],
            ..RoadmapData::default()
        },
        source: ReadSource::Memory,
    });
    let view = overview(&records);
    let data = &view.current().unwrap().value;
    assert_eq!(data.catalog.current["Desktop"].readiness.ready, 1);
    assert_eq!(data.catalog.history["Platform"].done, 1);
    assert_eq!(data.catalog.ungrouped.counts.total, 1);
    assert_eq!(data.frontiers.len(), 3);
    assert!(matches!(
        data.frontiers[0].items[0].readiness,
        WorkstreamReadiness::Ready
    ));
    assert!(data.frontiers[1].items.is_empty());
}

#[test]
fn missing_dependencies_and_failed_reads_cannot_look_ready() {
    let mut item = fixtures::chunk("dependent");
    item.deps = vec!["missing".into()];
    let mut records = ReadState::Current(Observed {
        value: RoadmapData {
            chunks: vec![item],
            ..RoadmapData::default()
        },
        source: ReadSource::Memory,
    });
    let view = overview(&records);
    let data = &view.current().unwrap().value;
    assert_eq!(data.catalog.ungrouped.readiness.ready, 0);
    assert!(matches!(&data.frontiers[0].items[0].readiness,
        WorkstreamReadiness::Waiting { dependencies } if dependencies[0].status.is_none()));
    records.record(Err(ReadError::new(
        ReadErrorKind::Io,
        "Cannot read the roadmap.",
    )));
    let stale = overview(&records);
    assert!(stale.current().is_err());
    assert_eq!(
        stale
            .last_successful()
            .unwrap()
            .value
            .catalog
            .ungrouped
            .readiness
            .waiting,
        1
    );
    assert!(matches!(
        overview(&ReadState::Unobserved),
        ReadState::Unobserved
    ));
}
