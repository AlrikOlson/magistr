use super::*;
use magistr_cockpit::harness::fixtures;
use magistr_workflow_contracts::plan::{TaskStatus, FeatureReadiness};
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

#[test]
fn overview_uses_complete_cross_feature_dependencies_and_keeps_history() {
    let mut dependent = fixtures::task("dependent");
    dependent.feature = Some("Desktop".into());
    dependent.deps = vec!["foundation".into()];
    let mut foundation = fixtures::task("foundation");
    foundation.feature = Some("Platform".into());
    foundation.status = TaskStatus::Done;
    let unassigned = fixtures::task("unassigned");
    let records = ReadState::Current(Observed {
        value: PlanData {
            tasks: vec![dependent, foundation, unassigned],
            ..PlanData::default()
        },
        source: ReadSource::Memory,
    });
    let view = overview(&records);
    let data = &view.current().unwrap().value;
    assert_eq!(data.catalog.current["Desktop"].readiness.ready, 1);
    assert_eq!(data.catalog.history["Platform"].done, 1);
    assert_eq!(data.catalog.unassigned.counts.total, 1);
    assert_eq!(data.frontiers.len(), 3);
    assert!(matches!(
        data.frontiers[0].items[0].readiness,
        FeatureReadiness::Ready
    ));
    assert!(data.frontiers[1].items.is_empty());
}

#[test]
fn missing_dependencies_and_failed_reads_cannot_look_ready() {
    let mut item = fixtures::task("dependent");
    item.deps = vec!["missing".into()];
    let mut records = ReadState::Current(Observed {
        value: PlanData {
            tasks: vec![item],
            ..PlanData::default()
        },
        source: ReadSource::Memory,
    });
    let view = overview(&records);
    let data = &view.current().unwrap().value;
    assert_eq!(data.catalog.unassigned.readiness.ready, 0);
    assert!(matches!(&data.frontiers[0].items[0].readiness,
        FeatureReadiness::Dependent { dependencies } if dependencies[0].status.is_none()));
    records.record(Err(ReadError::new(
        ReadErrorKind::Io,
        "Cannot read the plan.",
    )));
    let stale = overview(&records);
    assert!(stale.current().is_err());
    assert_eq!(
        stale
            .last_successful()
            .unwrap()
            .value
            .catalog
            .unassigned
            .readiness
            .dependent,
        1
    );
    assert!(matches!(
        overview(&ReadState::Unobserved),
        ReadState::Unobserved
    ));
}
