use super::*;
#[test]
fn legacy_history_does_not_invent_session_or_project_metadata() {
    let history: ThinkHistory =
        serde_json::from_value(serde_json::json!({"steps":[],"completed":false})).unwrap();
    assert!(
        history.metadata.is_none() && history.branches.is_none() && history.session_id.is_none()
    );
    assert_eq!(
        serde_json::to_value(history).unwrap(),
        serde_json::json!({"steps":[],"completed":false})
    );
    let metadata: HistoryMetadata = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(metadata.project_id, None);
}
