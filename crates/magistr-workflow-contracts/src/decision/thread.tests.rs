use super::*;
#[test]
fn legacy_thread_has_no_invented_merge_target() {
    let thread: Thread = serde_json::from_value(serde_json::json!({
        "id":"b","name":"Question","from_note":3,"notes":[],"status":"merged","created_at":"then","depth":1
    })).unwrap();
    assert_eq!(thread.status, ThreadStatus::Merged);
    assert_eq!(thread.merged_into, None);
    assert!(
        serde_json::to_value(thread)
            .unwrap()
            .get("merged_into")
            .is_none()
    );
}
