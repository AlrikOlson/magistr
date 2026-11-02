use super::*;
const OLD: &[u8] = include_bytes!("../fixtures/older.json");

#[test]
fn omitted_item_origins_are_filled_once_and_authored_content_survives() {
    let bytes = SignalEngine::upgrade_document(OLD, "saved-project").unwrap();
    let mut expected: Value = serde_json::from_slice(OLD).unwrap();
    let signal = expected["signals"][0].as_object_mut().unwrap();
    // Saved before the plan words: the plan task reference and its field
    // take the new spellings.
    let refs = signal.remove("cross_refs").unwrap();
    assert_eq!(refs, json!(["chunk:saved"]));
    signal.insert("refs".into(), json!(["task:saved"]));
    signal.insert("project_id".into(), json!("saved-project"));
    expected["schema_version"] = json!(3);
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
    assert!(bytes.starts_with(b"{\"schema_version\":3"));
    assert_eq!(
        SignalEngine::upgrade_document(&bytes, "saved-project").unwrap(),
        bytes
    );
}

#[test]
fn foreign_origins_and_duplicate_items_are_never_relabelled() {
    assert!(SignalEngine::upgrade_document(OLD, "another-project").is_err());
    let mut value: Value = serde_json::from_slice(OLD).unwrap();
    value["signals"][0]["project_id"] = json!("another-project");
    assert!(
        SignalEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "saved-project")
            .is_err()
    );
    let mut value: Value = serde_json::from_slice(OLD).unwrap();
    let duplicate = value["signals"][0].clone();
    value["signals"].as_array_mut().unwrap().push(duplicate);
    assert!(
        SignalEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "saved-project")
            .is_err()
    );
}

#[test]
fn signals_saved_before_the_decision_words_take_them_once() {
    let mut v2: Value =
        serde_json::from_slice(&SignalEngine::upgrade_document(OLD, "saved-project").unwrap())
            .unwrap();
    v2["schema_version"] = json!(2);
    v2["signals"][0]["refs"] = json!(["task:saved", "think:56"]);
    v2["signals"][0]["enrichment"] = json!([{
        "think_step": 57, "sources": [], "summary": "Recorded research.",
        "confidence": 0.9, "at": "2026-10-25T09:41:47Z"
    }]);
    let bytes =
        SignalEngine::upgrade_document(&serde_json::to_vec(&v2).unwrap(), "saved-project").unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(new["schema_version"], 3);
    assert_eq!(
        new["signals"][0]["refs"],
        json!(["task:saved", "decision:56"])
    );
    assert_eq!(new["signals"][0]["enrichment"][0]["note"], 57);
    assert!(
        new["signals"][0]["enrichment"][0]
            .get("think_step")
            .is_none()
    );
    assert_eq!(
        SignalEngine::upgrade_document(&bytes, "saved-project").unwrap(),
        bytes
    );
}
