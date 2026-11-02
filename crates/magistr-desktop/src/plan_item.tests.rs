use super::*;
use magistr_cockpit::harness::fixtures;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};
#[test]
fn item_read_preserves_complete_authored_fields_and_missing_or_stale_state() {
    let mut task = fixtures::task("chosen");
    task.description =
        "**Read** the [guide](https://example.com). <script>alert(1)</script>".into();
    task.acceptance = vec!["Keep the **whole** draft.".into()];
    task.notes = "日本語\n".repeat(5000);
    task.content = Some(serde_json::from_value(serde_json::json!({
        "version": 1, "summary": "Keep the complete plan.",
        "sections": [{"heading": "Details", "prose": "Keep **all** details.", "list": [{"text": "Read the plan.", "done": true}]}]
    })).unwrap());
    let mut read = ReadState::Current(Observed {
        value: PlanData {
            tasks: vec![task.clone()],
            ..PlanData::default()
        },
        source: ReadSource::Memory,
    });
    let item = selected(&read, "chosen");
    let saved = item.current().unwrap().value.as_ref().unwrap();
    assert_eq!(saved.record, task);
    assert_eq!(saved.requirements.len(), 1);
    assert_eq!(saved.sections.len(), 1);
    assert!(!saved.sections[0].nodes.is_empty());
    assert!(!saved.description.nodes.is_empty());
    assert!(
        selected(&read, "missing")
            .current()
            .unwrap()
            .value
            .is_none()
    );
    read.record(Err(ReadError::new(
        ReadErrorKind::Io,
        "Cannot read the plan.",
    )));
    let stale = selected(&read, "chosen");
    assert!(stale.current().is_err());
    assert_eq!(
        stale
            .last_successful()
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .record,
        task
    );
    assert!(matches!(
        selected(&ReadState::Unobserved, "chosen"),
        ReadState::Unobserved
    ));
}
#[test]
fn rich_presentation_limits_keep_all_original_source() {
    let mut task = fixtures::task("large");
    task.description = "- Full line\n".repeat(5000);
    let view = PlanItem::new(task.clone(), std::slice::from_ref(&task));
    assert!(view.description.limited);
    assert_eq!(view.description.nodes[0].text, task.description);
    assert_eq!(view.record.description, task.description);
}

#[test]
fn relations_resolve_the_complete_graph_and_keep_missing_records_explicit() {
    let mut chosen = fixtures::task("chosen");
    chosen.feature = Some("Desktop".into());
    chosen.deps = vec!["foundation".into(), "missing".into()];
    let mut foundation = fixtures::task("foundation");
    foundation.feature = Some("Platform".into());
    foundation.status = TaskStatus::Done;
    let mut dependent = fixtures::task("dependent");
    dependent.feature = Some("Delivery".into());
    dependent.deps = vec!["chosen".into()];
    let records = vec![chosen.clone(), foundation.clone(), dependent];
    let item = PlanItem::new(chosen, &records);
    assert!(
        matches!(&item.readiness, WorkstreamReadiness::Dependent { dependencies }
        if dependencies.len() == 1 && dependencies[0].id == "missing")
    );
    assert_eq!(item.dependencies.len(), 2);
    assert_eq!(item.dependencies[0].title.as_ref(), Some(&foundation.title));
    assert_eq!(item.dependencies[0].status, Some(TaskStatus::Done));
    assert_eq!(item.dependencies[0].feature.as_deref(), Some("Platform"));
    assert!(item.dependencies[1].title.is_none());
    assert!(item.dependencies[1].status.is_none());
    assert_eq!(item.dependents[0].id, "dependent");
    assert_eq!(item.dependents[0].feature.as_deref(), Some("Delivery"));
}
