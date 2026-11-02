use super::*;
use crate::decision::config::DecisionConfig;
use crate::decision::engine::ReasoningServer;
use tempfile::tempdir;

fn note(log: Option<&str>) -> crate::decision::domain::Note {
    serde_json::from_value(serde_json::json!({
        "estimated_total": 10, "purpose": "analysis", "context": "current project",
        "thought": "Check the saved history", "outcome": "evidence", "next_action": "continue",
        "rationale": "verify", "log_id": log
    }))
    .unwrap()
}

fn config(path: &Path) -> DecisionConfig {
    let mut config = DecisionConfig::default();
    config.persistence.enabled = true;
    config.persistence.data_dir = path.to_path_buf();
    config.features.enable_logs = true;
    config
}

#[test]
fn project_observation_preserves_absence_schema_decode_and_identity_refusals() {
    let directory = tempdir().unwrap();
    let config = config(directory.path());
    let owner = Persistence::for_project(&config.persistence, "project");
    assert_eq!(owner.load_default().unwrap().source, ReadSource::Missing);
    ReasoningServer::new_for_project(config, "project".into())
        .process_note_native(note(None))
        .unwrap();
    let path = owner.store_path().unwrap();
    let good = std::fs::read(&path).unwrap();
    let mut bad: serde_json::Value = serde_json::from_slice(&good).unwrap();
    bad["schema_version"] = serde_json::json!(999);
    std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert_eq!(
        owner.load_default().unwrap_err().kind,
        ReadErrorKind::Schema
    );
    std::fs::write(&path, "{").unwrap();
    assert_eq!(
        owner.load_default().unwrap_err().kind,
        ReadErrorKind::Decode
    );
    let mut wrong: serde_json::Value = serde_json::from_slice(&good).unwrap();
    wrong["project_id"] = serde_json::json!("another");
    std::fs::write(&path, serde_json::to_vec(&wrong).unwrap()).unwrap();
    assert_eq!(
        owner.load_default().unwrap_err().kind,
        ReadErrorKind::Identity
    );
    std::fs::write(&path, good).unwrap();
    assert!(matches!(
        owner.load_default().unwrap().source,
        ReadSource::File(_)
    ));
}

#[test]
fn disabled_and_invalid_project_paths_are_explicit() {
    let directory = tempdir().unwrap();
    let mut config = config(directory.path());
    config.persistence.enabled = false;
    let owner = Persistence::for_project(&config.persistence, "project");
    assert_eq!(owner.load_default().unwrap().source, ReadSource::Memory);
    config.persistence.enabled = true;
    assert_eq!(
        Persistence::for_project(&config.persistence, "../outside")
            .load_project()
            .unwrap_err()
            .kind,
        ReadErrorKind::Identity
    );
    assert!(!directory.path().join("decisions").exists());
}

#[test]
fn named_logs_are_projections_of_one_validated_document() {
    let directory = tempdir().unwrap();
    let config = config(directory.path());
    let owner = Persistence::for_project(&config.persistence, "project");
    let mut writer = ReasoningServer::new_for_project(config, "project".into());
    let first = writer.process_note_native(note(Some("review"))).unwrap();
    let second = writer.process_note_native(note(Some("build"))).unwrap();
    assert_eq!(first.note.note, 1);
    assert_eq!(second.note.note, 2);
    let review = owner.load_log("project__review").unwrap();
    let absent = owner.load_log("project__absent").unwrap();
    assert_eq!(review.source, absent.source);
    assert!(absent.value.is_none());
    assert_eq!(
        review.value.unwrap().notes[0].record_id,
        first.note.record_id
    );
    assert_eq!(owner.load_logs().unwrap().len(), 2);
    assert_eq!(
        owner.load_log("other__review").unwrap_err().kind,
        ReadErrorKind::Identity
    );
    let files: Vec<_> = std::fs::read_dir(owner.documents_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), 2, "one project document plus its stable lock");
}

#[test]
fn duplicate_numbers_across_logs_refuse_the_entire_project() {
    let directory = tempdir().unwrap();
    let config = config(directory.path());
    let owner = Persistence::for_project(&config.persistence, "project");
    let mut writer = ReasoningServer::new_for_project(config, "project".into());
    writer.process_note_native(note(Some("first"))).unwrap();
    writer.process_note_native(note(Some("second"))).unwrap();
    let path = owner.store_path().unwrap();
    let mut bad: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    bad["logs"]["project__second"]["notes"][0]["note"] = serde_json::json!(1);
    let bytes = serde_json::to_vec(&bad).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        owner.load_log("project__first").unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert!(writer.process_note_native(note(Some("first"))).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
