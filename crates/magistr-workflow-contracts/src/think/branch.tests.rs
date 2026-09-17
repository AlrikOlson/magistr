use super::*;
#[test]
fn legacy_branch_has_no_invented_merge_target() {
    let branch: Branch = serde_json::from_value(serde_json::json!({
        "id":"b","name":"Question","from_step":3,"steps":[],"status":"merged","created_at":"then","depth":1
    })).unwrap();
    assert_eq!(branch.status, BranchStatus::Merged);
    assert_eq!(branch.merged_into, None);
    assert!(
        serde_json::to_value(branch)
            .unwrap()
            .get("merged_into")
            .is_none()
    );
}
