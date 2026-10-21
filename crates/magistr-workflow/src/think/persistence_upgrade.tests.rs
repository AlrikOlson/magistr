use super::*;
use crate::infra::PersistenceConfig;
use std::path::PathBuf;
const OLD: &[u8] = include_bytes!("fixtures/v1.json");

fn owner() -> Persistence {
    Persistence::for_project(
        &PersistenceConfig {
            enabled: false,
            data_dir: PathBuf::new(),
        },
        "saved-project",
    )
}

#[test]
fn older_default_and_named_histories_are_consolidated_without_losing_steps() {
    let mut named: Value = serde_json::from_slice(OLD).unwrap();
    named["history"]["session_id"] = json!("saved-project__review");
    named["history"]["steps"][0]["step_number"] = json!(8);
    named["history"]["steps"][0]
        .as_object_mut()
        .unwrap()
        .remove("record_id");
    let documents = BTreeMap::from([
        ("saved-project".into(), OLD.to_vec()),
        (
            "saved-project__review".into(),
            serde_json::to_vec(&named).unwrap(),
        ),
    ]);
    let bytes = owner().upgrade_documents(&documents).unwrap();
    let converted: PersistedProject = serde_json::from_slice(&bytes).unwrap();
    owner().validate(&converted).unwrap();
    assert_eq!(converted.histories.len(), 2);
    let old: Value = serde_json::from_slice(OLD).unwrap();
    let expected: ThinkHistory = serde_json::from_value(old["history"].clone()).unwrap();
    assert_eq!(converted.histories["saved-project"], expected);
    assert_eq!(
        converted.histories["saved-project__review"].steps[0].step_number,
        8
    );
    assert!(
        converted.histories["saved-project__review"].steps[0]
            .record_id
            .is_some()
    );
    assert_eq!(owner().upgrade_documents(&documents).unwrap(), bytes);
    let current = BTreeMap::from([("saved-project".into(), bytes.clone())]);
    assert_eq!(owner().upgrade_documents(&current).unwrap(), bytes);
}

#[test]
fn inconsistent_histories_are_refused_instead_of_silently_renumbered() {
    for change in ["version", "project", "session", "number", "identity"] {
        let mut bad: Value = serde_json::from_slice(OLD).unwrap();
        match change {
            "version" => bad["schema_version"] = json!(99),
            "project" => bad["history"]["metadata"]["project_id"] = json!("another"),
            "session" => bad["history"]["session_id"] = json!("another__review"),
            "number" => {
                let step = bad["history"]["steps"][0].clone();
                bad["history"]["steps"].as_array_mut().unwrap().push(step);
            }
            _ => bad["history"]["steps"][0]["record_id"] = json!(""),
        }
        let documents =
            BTreeMap::from([("saved-project".into(), serde_json::to_vec(&bad).unwrap())]);
        assert!(owner().upgrade_documents(&documents).is_err(), "{change}");
    }
}
