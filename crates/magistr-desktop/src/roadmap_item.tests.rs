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
    let view = RoadmapItem::new(chunk.clone());
    assert!(view.description.limited);
    assert_eq!(view.description.nodes[0].text, chunk.description);
    assert_eq!(view.record.description, chunk.description);
}
