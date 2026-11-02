use super::*;
/// Saved before the plan words: no version, `signals`, `cross_refs`.
const OLD: &[u8] = include_bytes!("../fixtures/older.json");
/// A real document saved after the decision words, before the request words
/// (version 3), trimmed: kinds `idea` and `bug`, statuses `surfaced`,
/// `researched`, `dismissed` and `new`, a `surfaced_at` stamp and saved
/// references. Its free text is replaced because it came from another
/// project.
const DECISION_WORDS: &[u8] = include_bytes!("../fixtures/before-request-words.json");

#[test]
fn omitted_item_origins_are_filled_once_and_authored_content_survives() {
    let bytes = RequestEngine::upgrade_document(OLD, "saved-project").unwrap();
    let mut expected: Value = serde_json::from_slice(OLD).unwrap();
    let mut requests = expected.as_object_mut().unwrap().remove("signals").unwrap();
    let request = requests[0].as_object_mut().unwrap();
    // Saved before the plan words: the plan task reference and its field
    // take the new spellings.
    let refs = request.remove("cross_refs").unwrap();
    assert_eq!(refs, json!(["chunk:saved"]));
    request.insert("refs".into(), json!(["task:saved"]));
    request.insert("project_id".into(), json!("saved-project"));
    expected["requests"] = requests;
    expected["schema_version"] = json!(4);
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
    assert!(bytes.starts_with(b"{\"schema_version\":4"));
    assert_eq!(
        RequestEngine::upgrade_document(&bytes, "saved-project").unwrap(),
        bytes
    );
}

#[test]
fn foreign_origins_and_duplicate_items_are_never_relabelled() {
    assert!(RequestEngine::upgrade_document(OLD, "another-project").is_err());
    let mut value: Value = serde_json::from_slice(OLD).unwrap();
    value["signals"][0]["project_id"] = json!("another-project");
    assert!(
        RequestEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "saved-project")
            .is_err()
    );
    let mut value: Value = serde_json::from_slice(OLD).unwrap();
    let duplicate = value["signals"][0].clone();
    value["signals"].as_array_mut().unwrap().push(duplicate);
    assert!(
        RequestEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "saved-project")
            .is_err()
    );
}

#[test]
fn requests_saved_before_the_decision_words_take_them_once() {
    let current = RequestEngine::upgrade_document(OLD, "saved-project").unwrap();
    let mut v2: Value = serde_json::from_slice(&current).unwrap();
    let items = v2.as_object_mut().unwrap().remove("requests").unwrap();
    v2["signals"] = items;
    v2["schema_version"] = json!(2);
    v2["signals"][0]["refs"] = json!(["task:saved", "think:56", "signal:other"]);
    v2["signals"][0]["enrichment"] = json!([{
        "think_step": 57, "sources": [], "summary": "Recorded research.",
        "confidence": 0.9, "at": "2026-10-25T09:41:47Z"
    }]);
    let bytes = RequestEngine::upgrade_document(&serde_json::to_vec(&v2).unwrap(), "saved-project")
        .unwrap();
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(new["schema_version"], 4);
    assert_eq!(
        new["requests"][0]["refs"],
        json!(["task:saved", "decision:56", "request:other"])
    );
    assert_eq!(new["requests"][0]["enrichment"][0]["note"], 57);
    assert!(
        new["requests"][0]["enrichment"][0]
            .get("think_step")
            .is_none()
    );
    assert_eq!(
        RequestEngine::upgrade_document(&bytes, "saved-project").unwrap(),
        bytes
    );
}

#[test]
fn requests_saved_before_the_request_words_take_them_once() {
    let bytes = RequestEngine::upgrade_document(DECISION_WORDS, "magistr-5a5bc6").unwrap();
    assert!(bytes.starts_with(b"{\"schema_version\":4"));
    let new: Value = serde_json::from_slice(&bytes).unwrap();
    let old: Value = serde_json::from_slice(DECISION_WORDS).unwrap();
    assert!(new.get("signals").is_none());
    let (old, new) = (
        old["signals"].as_array().unwrap(),
        new["requests"].as_array().unwrap(),
    );
    assert_eq!(old.len(), new.len());
    for (old, new) in old.iter().zip(new) {
        let kind = match old["kind"].as_str().unwrap() {
            "idea" => "suggestion",
            kind => kind,
        };
        let status = match old["status"].as_str().unwrap() {
            "surfaced" => "raised",
            status => status,
        };
        assert_eq!(new["kind"], kind);
        assert_eq!(new["status"], status);
        assert_eq!(new["raised_at"], old["surfaced_at"]);
        assert!(new.get("surfaced_at").is_none());
        assert_eq!(new["refs"], old["refs"], "no request reference was saved");
        assert_eq!(new["enrichment"], old["enrichment"]);
        assert_eq!(new["body"], old["body"]);
    }
    let raised = new.iter().filter(|r| r["status"] == "raised").count();
    let suggestions = new.iter().filter(|r| r["kind"] == "suggestion").count();
    assert!(raised > 0 && suggestions > 0, "the fixture covers both");
    assert_eq!(
        RequestEngine::upgrade_document(&bytes, "magistr-5a5bc6").unwrap(),
        bytes,
        "a second pass changes nothing"
    );
}

#[test]
fn a_current_document_with_old_words_is_refused_not_translated() {
    let current = RequestEngine::upgrade_document(DECISION_WORDS, "magistr-5a5bc6").unwrap();
    let mut value: Value = serde_json::from_slice(&current).unwrap();
    value["requests"][0]["kind"] = json!("idea");
    assert!(
        RequestEngine::upgrade_document(&serde_json::to_vec(&value).unwrap(), "magistr-5a5bc6")
            .is_err()
    );
}
