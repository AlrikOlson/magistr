use super::*;
use serde_json::json;

#[test]
fn the_version_leads_and_the_rest_is_unchanged() {
    let value = json!({"tasks": [1], "project_id": "p", "schema_version": 1});
    let bytes = versioned_document(value, 2).unwrap();
    assert!(bytes.starts_with(b"{\"schema_version\":2,"));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"schema_version": 2, "tasks": [1], "project_id": "p"})
    );
}

#[test]
fn an_empty_document_still_names_its_version() {
    let bytes = versioned_document(json!({}), 4).unwrap();
    assert_eq!(bytes, b"{\"schema_version\":4}");
}

#[test]
fn a_document_that_is_not_an_object_is_refused() {
    assert!(versioned_document(json!([1]), 2).is_err());
}
