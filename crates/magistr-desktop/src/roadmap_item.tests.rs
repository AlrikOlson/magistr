use super::*;
use magistr_cockpit::harness::fixtures;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};
#[test]
fn item_read_preserves_complete_authored_fields_and_missing_or_stale_state() {
    let mut chunk = fixtures::chunk("chosen");
    chunk.description =
        "**Read** the [guide](https://example.com). <script>alert(1)</script>".into();
    chunk.acceptance = vec!["Keep the **whole** draft.".into()];
    chunk.notes = "日本語\n".repeat(5000);
    chunk.content = Some(serde_json::from_value(serde_json::json!({
        "version": 1, "summary": "Keep the complete plan.",
        "sections": [{"heading": "Details", "prose": "Keep **all** details.", "list": [{"text": "Read the plan.", "done": true}]}]
    })).unwrap());
    let mut read = ReadState::Current(Observed {
        value: RoadmapData {
            chunks: vec![chunk.clone()],
            ..RoadmapData::default()
        },
        source: ReadSource::Memory,
    });
    let item = selected(&read, "chosen");
    let saved = item.current().unwrap().value.as_ref().unwrap();
    assert_eq!(saved.record, chunk);
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
        "Cannot read the roadmap.",
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
        chunk
    );
    assert!(matches!(
        selected(&ReadState::Unobserved, "chosen"),
        ReadState::Unobserved
    ));
}
#[test]
fn rich_presentation_limits_keep_all_original_source() {
    let mut chunk = fixtures::chunk("large");
    chunk.description = "- Full line\n".repeat(5000);
    let view = RoadmapItem::new(chunk.clone(), std::slice::from_ref(&chunk));
    assert!(view.description.limited);
    assert_eq!(view.description.nodes[0].text, chunk.description);
    assert_eq!(view.record.description, chunk.description);
}

#[test]
fn relations_resolve_the_complete_graph_and_keep_missing_records_explicit() {
    let mut chosen = fixtures::chunk("chosen");
    chosen.group = Some("Desktop".into());
    chosen.deps = vec!["foundation".into(), "missing".into()];
    let mut foundation = fixtures::chunk("foundation");
    foundation.group = Some("Platform".into());
    foundation.status = ChunkStatus::Done;
    let mut dependent = fixtures::chunk("dependent");
    dependent.group = Some("Delivery".into());
    dependent.deps = vec!["chosen".into()];
    let records = vec![chosen.clone(), foundation.clone(), dependent];
    let item = RoadmapItem::new(chosen, &records);
    assert!(
        matches!(&item.readiness, WorkstreamReadiness::Waiting { dependencies }
        if dependencies.len() == 1 && dependencies[0].id == "missing")
    );
    assert_eq!(item.dependencies.len(), 2);
    assert_eq!(item.dependencies[0].title.as_ref(), Some(&foundation.title));
    assert_eq!(item.dependencies[0].status, Some(ChunkStatus::Done));
    assert_eq!(item.dependencies[0].group.as_deref(), Some("Platform"));
    assert!(item.dependencies[1].title.is_none());
    assert!(item.dependencies[1].status.is_none());
    assert_eq!(item.dependents[0].id, "dependent");
    assert_eq!(item.dependents[0].group.as_deref(), Some("Delivery"));
}
