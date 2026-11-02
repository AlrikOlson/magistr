use super::*;
/// Written by the version 1 writer: a task with an action and a check, keyed
/// `objective`, `tasks`, `actions`, `next_action_id`.
const OLD: &[u8] = include_bytes!("fixtures/v1.json");
/// Real version 2 records from this project's store, trimmed to two cycles:
/// a completed objective, and a saved cycle with a blocked and a completed task.
const V2: &[u8] = include_bytes!("fixtures/v2.json");
/// A real stored check result whose target still names a task.
const CHECK_RESULT: &[u8] = include_bytes!("fixtures/check-result-before-run-words.json");

#[test]
fn version_one_work_keeps_content_and_gets_stable_execution_identities() {
    assert!(Persistence::validate_document(OLD, "saved-project").is_err());
    let bytes = Persistence::upgrade_document(OLD, "saved-project").unwrap();
    let old: Value = serde_json::from_slice(OLD).unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(new["schema_version"], 4);
    assert_eq!(new["project_id"], old["project_id"]);
    assert_eq!(new["goal"], old["objective"]);
    assert_eq!(new["next_progress_id"], old["next_action_id"]);
    let step = &new["steps"][0];
    assert_eq!(step["id"], "one");
    assert_eq!(step["checks"], old["tasks"][0]["checks"]);
    let progress = &step["progress"][0];
    assert_eq!(progress["step_id"], "one");
    assert_eq!(progress["think_step"], 7);
    assert!(progress.get("deliberate_step").is_none());
    assert_eq!(
        new["execution"]["run_id"],
        "legacy:saved-project:2026-08-03T10:00:00Z"
    );
    assert_eq!(new["execution"]["steps"]["one"], "legacy:0:one");
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
fn version_two_work_takes_the_run_words_once() {
    assert!(Persistence::validate_document(V2, "magistr-5a5bc6").is_err());
    let bytes = Persistence::upgrade_document(V2, "magistr-5a5bc6").unwrap();
    let old: Value = serde_json::from_slice(V2).unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(new["schema_version"], 4);
    for retired in ["objective", "tasks", "next_action_id"] {
        assert!(new.get(retired).is_none(), "{retired}");
    }
    assert_eq!(old["objective"]["status"], "completed");
    assert_eq!(new["goal"]["status"], "finished");
    assert_eq!(new["goal"]["description"], old["objective"]["description"]);
    assert_eq!(new["execution"]["run_id"], old["execution"]["run_id"]);
    assert_eq!(new["execution"]["steps"], old["execution"]["tasks"]);
    assert_eq!(new["next_progress_id"], old["next_action_id"]);
    for (index, step) in new["steps"].as_array().unwrap().iter().enumerate() {
        let before = &old["tasks"][index];
        assert_eq!(before["status"], "completed");
        assert_eq!(step["status"], "done");
        assert_eq!(
            step["progress"].as_array().unwrap().len(),
            before["actions"].as_array().unwrap().len()
        );
        for progress in step["progress"].as_array().unwrap() {
            assert_eq!(progress["step_id"], step["id"]);
            assert!(progress.get("task_id").is_none());
        }
    }
    let saved = &new["saved_cycles"][0];
    let before = old["saved_cycles"][0]["tasks"].as_array().unwrap();
    let blocked = before
        .iter()
        .position(|t| t["status"] == "blocked")
        .unwrap();
    let completed = before
        .iter()
        .position(|t| t["status"] == "completed")
        .unwrap();
    let waiting = &saved["steps"][blocked];
    assert_eq!(waiting["status"], "waiting");
    assert!(
        waiting["waiting_reason"]
            .as_str()
            .is_some_and(|r| !r.is_empty())
    );
    assert_eq!(waiting["waiting_reason"], before[blocked]["blocked_reason"]);
    assert!(waiting.get("blocked_reason").is_none());
    assert_eq!(saved["steps"][completed]["status"], "done");
    assert_eq!(saved["goal"]["status"], "active");
    assert_eq!(
        Persistence::upgrade_document(&bytes, "magistr-5a5bc6").unwrap(),
        bytes,
        "converting twice changes nothing"
    );
}

#[test]
fn a_stored_check_result_targets_a_step() {
    let bytes = Persistence::upgrade_check_result(CHECK_RESULT).unwrap();
    let old: Value = serde_json::from_slice(CHECK_RESULT).unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(new["target"]["step_id"], old["target"]["task_id"]);
    assert_eq!(
        new["target"]["step_instance"],
        old["target"]["task_instance"]
    );
    assert!(new["target"].get("task_id").is_none());
    let result: crate::run::domain::check_result::CheckResult =
        serde_json::from_slice(&bytes).expect("the current reader accepts the converted record");
    assert_eq!(result.target.step_id, "design-01-source");
    assert!(
        serde_json::from_slice::<crate::run::domain::check_result::CheckResult>(CHECK_RESULT)
            .is_err(),
        "the current reader refuses the old spelling"
    );
    assert_eq!(Persistence::upgrade_check_result(&bytes).unwrap(), bytes);
}

#[test]
fn a_record_holding_both_spellings_is_refused() {
    let mut old: Value = serde_json::from_slice(V2).unwrap();
    old["goal"] = old["objective"].clone();
    assert!(
        Persistence::upgrade_document(&serde_json::to_vec(&old).unwrap(), "magistr-5a5bc6")
            .is_err()
    );
}

#[test]
fn saved_cycles_and_existing_check_result_identities_survive_conversion() {
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
        json!({"run_id":"saved-run","steps":{"one":"saved-instance"}})
    );
    assert_eq!(
        new["saved_cycles"][0]["steps"][0]["progress"][0]["think_step"],
        7
    );
    assert_eq!(new["selection_revision"], 9);
}

#[test]
fn unknown_corrupt_and_cross_project_work_is_never_repaired_by_defaults() {
    let original: Value = serde_json::from_slice(OLD).unwrap();
    for version in [0, 999] {
        let mut old = original.clone();
        old["schema_version"] = json!(version);
        assert!(
            Persistence::upgrade_document(&serde_json::to_vec(&old).unwrap(), "saved-project")
                .is_err()
        );
    }
    assert!(Persistence::upgrade_document(OLD, "another-project").is_err());
    assert!(Persistence::upgrade_document(V2, "another-project").is_err());
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
fn malformed_steps_and_conflicting_reasoning_links_are_refused_without_panicking() {
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

#[test]
fn version_three_work_names_plan_tasks_with_the_task_prefix_once() {
    let current = Persistence::upgrade_document(V2, "magistr-5a5bc6").unwrap();
    let mut v3: Value = serde_json::from_slice(&current).unwrap();
    v3["schema_version"] = json!(3);
    v3["goal"]["scope"] = json!("chunk:saved-chunk");
    let upgraded =
        Persistence::upgrade_document(&serde_json::to_vec(&v3).unwrap(), "magistr-5a5bc6").unwrap();
    let new: Value = serde_json::from_slice(&upgraded).unwrap();
    assert_eq!(new["schema_version"], 4);
    assert_eq!(new["goal"]["scope"], "task:saved-chunk");
    assert_eq!(
        Persistence::upgrade_document(&upgraded, "magistr-5a5bc6").unwrap(),
        upgraded,
        "a current document is unchanged"
    );
}
