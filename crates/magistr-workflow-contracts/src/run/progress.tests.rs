use super::*;
#[test]
fn legacy_progress_default_optional_evidence_without_inventing_it() {
    let progress: Progress = serde_json::from_value(serde_json::json!({
        "id":1,"step_id":"t","timestamp":"then","type":"review","description":"Reviewed","result":"OK"
    })).unwrap();
    assert!(progress.files_touched.is_empty() && progress.tools_used.is_empty());
    assert_eq!(progress.note, None);
    assert_eq!(progress.progress_type.as_wire(), "review");
    assert_eq!(serde_json::to_value(progress).unwrap()["type"], "review");
}
