use super::*;
const OLD: &[u8] = include_bytes!("fixtures/v1.json");

#[test]
fn version_one_work_keeps_content_and_gets_stable_execution_identities() {
    assert!(Persistence::validate_document(OLD, "saved-project").is_err());
    let bytes = Persistence::upgrade_document(OLD, "saved-project").unwrap();
    let mut old: Value = serde_json::from_slice(OLD).unwrap();
    let step = old["tasks"][0]["actions"][0]
        .as_object_mut()
        .unwrap()
        .remove("deliberate_step")
        .unwrap();
    old["tasks"][0]["actions"][0]["think_step"] = step;
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    for field in ["project_id", "objective", "tasks", "next_action_id"] {
        assert_eq!(new[field], old[field], "{field}");
    }
    assert_eq!(
        new["execution"]["run_id"],
        "legacy:saved-project:2026-08-03T10:00:00Z"
    );
    assert_eq!(new["execution"]["tasks"]["one"], "legacy:0:one");
    assert_eq!(
        Persistence::upgrade_document(OLD, "saved-project").unwrap(),
        bytes
    );
    assert_eq!(
        Persistence::upgrade_document(&bytes, "saved-project").unwrap(),
        bytes
    );
}

#[test]
fn saved_cycles_and_existing_receipt_identities_survive_conversion() {
    let mut old: Value = serde_json::from_slice(OLD).unwrap();
    let mut retained = old.clone();
    retained["execution"] = json!({"run_id":"saved-run","tasks":{"one":"saved-instance"}});
    old["saved_cycles"] = json!([retained]);
    old["selection_revision"] = json!(9);
    let bytes =
        Persistence::upgrade_document(&serde_json::to_vec(&old).unwrap(), "saved-project").unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        new["saved_cycles"][0]["execution"],
        old["saved_cycles"][0]["execution"]
    );
    assert_eq!(
        new["saved_cycles"][0]["tasks"][0]["actions"][0]["think_step"],
        7
    );
    assert_eq!(new["selection_revision"], 9);
}

#[test]
fn unknown_corrupt_and_cross_project_work_is_never_repaired_by_defaults() {
    let original: Value = serde_json::from_slice(OLD).unwrap();
    for version in [2, 999] {
        let mut old = original.clone();
        old["schema_version"] = json!(version);
        assert!(
            Persistence::upgrade_document(&serde_json::to_vec(&old).unwrap(), "saved-project")
                .is_err()
        );
    }
    assert!(Persistence::upgrade_document(OLD, "another-project").is_err());
    let mut duplicate = original.clone();
    duplicate["tasks"]
        .as_array_mut()
        .unwrap()
        .push(original["tasks"][0].clone());
    assert!(
        Persistence::upgrade_document(&serde_json::to_vec(&duplicate).unwrap(), "saved-project")
            .is_err()
    );
    let mut bad_identity = original;
    bad_identity["execution"] = json!({"run_id":"existing","tasks":{}});
    assert!(
        Persistence::upgrade_document(&serde_json::to_vec(&bad_identity).unwrap(), "saved-project")
            .is_err()
    );
}
#[test]
fn malformed_tasks_and_conflicting_reasoning_links_are_refused_without_panicking() {
    for tasks in [
        json!([null]),
        json!([{"actions": [false]}]),
        json!("broken"),
    ] {
        let mut old: Value = serde_json::from_slice(OLD).unwrap();
        old["tasks"] = tasks;
        assert!(
            Persistence::upgrade_document(&serde_json::to_vec(&old).unwrap(), "saved-project")
                .is_err()
        );
    }
    let mut old: Value = serde_json::from_slice(OLD).unwrap();
    old["tasks"][0]["actions"][0]["think_step"] = json!(8);
    assert!(
        Persistence::upgrade_document(&serde_json::to_vec(&old).unwrap(), "saved-project").is_err()
    );
}
