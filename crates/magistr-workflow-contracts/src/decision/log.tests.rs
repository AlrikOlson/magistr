use super::*;
#[test]
fn legacy_history_does_not_invent_log_or_project_metadata() {
    let history: Log =
        serde_json::from_value(serde_json::json!({"notes":[],"completed":false})).unwrap();
    assert!(history.metadata.is_none() && history.threads.is_none() && history.log_id.is_none());
    assert_eq!(
        serde_json::to_value(history).unwrap(),
        serde_json::json!({"notes":[],"completed":false})
    );
    let metadata: LogMetadata = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(metadata.project_id, None);
}
