use super::*;
/// Written before the plan words: `chunks`, `pending`, `cross_refs`, no version.
const OLD: &[u8] = include_bytes!("../fixtures/older.json");

fn converted(bytes: &[u8]) -> Value {
    serde_json::from_slice(&PlanEngine::upgrade_document(bytes, "saved-project").unwrap()).unwrap()
}

#[test]
fn omitted_item_origins_are_filled_once_and_authored_content_survives() {
    let bytes = PlanEngine::upgrade_document(OLD, "saved-project").unwrap();
    let old: Value = serde_json::from_slice(OLD).unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(new["schema_version"], 2);
    assert!(new.get("chunks").is_none());
    let (before, after) = (&old["chunks"][0], &new["tasks"][0]);
    assert_eq!(after["project_id"], "saved-project");
    assert_eq!(after["status"], "to_do");
    assert_eq!(after["refs"], before["cross_refs"]);
    for authored in ["id", "title", "name", "description", "notes", "acceptance"] {
        assert_eq!(after[authored], before[authored], "{authored}");
    }
    assert_eq!(new["preamble"], old["preamble"]);
    assert!(
        bytes.starts_with(b"{\"schema_version\":2"),
        "the version leads the document"
    );
    assert_eq!(
        PlanEngine::upgrade_document(&bytes, "saved-project").unwrap(),
        bytes,
        "a current document is unchanged"
    );
}

#[test]
fn every_stored_plan_word_takes_its_new_spelling() {
    let mut old: Value = serde_json::from_slice(OLD).unwrap();
    let template = old["chunks"][0].clone();
    let mut chunks = Vec::new();
    for (id, status) in [
        ("a", "backlog"),
        ("b", "blocked"),
        ("c", "obsoleted"),
        ("d", "in_progress"),
    ] {
        let mut chunk = template.clone();
        chunk["id"] = json!(id);
        chunk["status"] = json!(status);
        chunk["group"] = json!("Platform");
        chunks.push(chunk);
    }
    chunks[1]["blocked_by"] = json!({
        "kind": "premise_unmet", "reason": "no tenant yet",
        "blocked_at": "2026-08-03T09:00:00Z"
    });
    chunks[2]["obsoleted_reason"] = json!("Overtaken");
    chunks[3]["cross_refs"] = json!(["chunk:a", "task:verify", "action:3", "step:9"]);
    chunks[3]["status_proposal"] = json!({
        "suggested_status": "pending", "reason": "r",
        "proposed_at": "2026-08-03T09:00:00Z", "source": "s"
    });
    old["chunks"] = json!(chunks);
    old["links"] = json!([{"chunk_id": "a", "provider": "github", "external_id": "1",
        "our_last_write_hash": "h", "created_at": "2026-08-03T09:00:00Z",
        "updated_at": "2026-08-03T09:00:00Z"}]);
    old["tracker_opt_ins"] = json!([{"chunk_id": "a", "provider": "github",
        "enabled": true, "updated_at": "2026-08-03T09:00:00Z"}]);
    old["container_links"] = json!([{"kind": "group", "name": "Platform",
        "provider": "github", "external_id": "m1", "created_by_us": true,
        "created_at": "2026-08-03T09:00:00Z", "updated_at": "2026-08-03T09:00:00Z"}]);
    old["focuses"] = json!([{"lane": "/work", "group": "Platform", "mode": "build",
        "set_at": "2026-08-03T09:00:00Z", "updated_at": "2026-08-03T09:00:00Z"}]);

    let new = converted(&serde_json::to_vec(&old).unwrap());
    let tasks = new["tasks"].as_array().unwrap();
    let statuses: Vec<&str> = tasks
        .iter()
        .map(|t| t["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["idea", "waiting", "retired", "in_progress"]);
    assert!(
        tasks
            .iter()
            .all(|t| t["feature"] == "Platform" && t.get("group").is_none())
    );
    assert_eq!(
        tasks[1]["waiting_on"],
        json!({"kind": "condition", "reason": "no tenant yet",
            "waiting_since": "2026-08-03T09:00:00Z"})
    );
    assert_eq!(tasks[2]["retired_reason"], "Overtaken");
    assert_eq!(
        tasks[3]["refs"],
        json!(["task:a", "step:verify", "progress:3", "think:9"]),
        "run references take the run words and plan tasks take task: in one pass"
    );
    assert_eq!(tasks[3]["status_proposal"]["suggested_status"], "to_do");
    assert_eq!(new["links"][0]["task_id"], "a");
    assert_eq!(new["tracker_opt_ins"][0]["task_id"], "a");
    assert_eq!(new["container_links"][0]["kind"], "feature");
    assert_eq!(new["focuses"][0]["feature"], "Platform");
    let typed: Plan = serde_json::from_value(new).unwrap();
    assert_eq!(
        typed.tasks[1].status,
        magistr_workflow_contracts::TaskStatus::Waiting
    );
}

#[test]
fn a_record_holding_both_spellings_is_refused() {
    let mut old: Value = serde_json::from_slice(OLD).unwrap();
    old["chunks"][0]["refs"] = json!([]);
    assert!(
        PlanEngine::upgrade_document(&serde_json::to_vec(&old).unwrap(), "saved-project").is_err()
    );
    let mut future: Value = serde_json::from_slice(OLD).unwrap();
    future["schema_version"] = json!(99);
    assert!(
        PlanEngine::upgrade_document(&serde_json::to_vec(&future).unwrap(), "saved-project")
            .is_err()
    );
}

#[test]
fn foreign_origins_and_duplicate_items_are_never_relabelled() {
    assert!(PlanEngine::upgrade_document(OLD, "another-project").is_err());
    let mut value: Value = serde_json::from_slice(OLD).unwrap();
    value["chunks"][0]["project_id"] = json!("another-project");
    assert!(
        PlanEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "saved-project")
            .is_err()
    );
    let mut value: Value = serde_json::from_slice(OLD).unwrap();
    let duplicate = value["chunks"][0].clone();
    value["chunks"].as_array_mut().unwrap().push(duplicate);
    assert!(
        PlanEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "saved-project")
            .is_err()
    );
}
