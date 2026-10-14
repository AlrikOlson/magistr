use super::*;
use crate::think::config::ThinkConfig;
use crate::think::engine::ReasoningServer;
use tempfile::tempdir;

fn step(session: Option<&str>) -> crate::think::domain::ThinkStep {
    serde_json::from_value(serde_json::json!({
        "estimated_total": 10, "purpose": "analysis", "context": "current project",
        "thought": "Check the saved history", "outcome": "evidence", "next_action": "continue",
        "rationale": "verify", "session_id": session
    }))
    .unwrap()
}

fn config(path: &Path) -> ThinkConfig {
    let mut config = ThinkConfig::default();
    config.persistence.enabled = true;
    config.persistence.data_dir = path.to_path_buf();
    config.features.enable_sessions = true;
    config
}

#[test]
fn project_observation_preserves_absence_schema_decode_and_identity_refusals() {
    let directory = tempdir().unwrap();
    let config = config(directory.path());
    let owner = Persistence::for_project(&config.persistence, "project");
    assert_eq!(owner.load_default().unwrap().source, ReadSource::Missing);
    ReasoningServer::new_for_project(config, "project".into())
        .process_step_native(step(None))
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
    assert!(!directory.path().join("think").exists());
}

#[test]
fn named_histories_are_projections_of_one_validated_document() {
    let directory = tempdir().unwrap();
    let config = config(directory.path());
    let owner = Persistence::for_project(&config.persistence, "project");
    let mut writer = ReasoningServer::new_for_project(config, "project".into());
    let first = writer.process_step_native(step(Some("review"))).unwrap();
    let second = writer.process_step_native(step(Some("build"))).unwrap();
    assert_eq!(first.step.step_number, 1);
    assert_eq!(second.step.step_number, 2);
    let review = owner.load_session("project__review").unwrap();
    let absent = owner.load_session("project__absent").unwrap();
    assert_eq!(review.source, absent.source);
    assert!(absent.value.is_none());
    assert_eq!(
        review.value.unwrap().steps[0].record_id,
        first.step.record_id
    );
    assert_eq!(owner.load_sessions().unwrap().len(), 2);
    assert_eq!(
        owner.load_session("other__review").unwrap_err().kind,
        ReadErrorKind::Identity
    );
    let files: Vec<_> = std::fs::read_dir(owner.sessions_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), 2, "one project document plus its stable lock");
}

#[test]
fn duplicate_numbers_across_histories_refuse_the_entire_project() {
    let directory = tempdir().unwrap();
    let config = config(directory.path());
    let owner = Persistence::for_project(&config.persistence, "project");
    let mut writer = ReasoningServer::new_for_project(config, "project".into());
    writer.process_step_native(step(Some("first"))).unwrap();
    writer.process_step_native(step(Some("second"))).unwrap();
    let path = owner.store_path().unwrap();
    let mut bad: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    bad["histories"]["project__second"]["steps"][0]["step_number"] = serde_json::json!(1);
    let bytes = serde_json::to_vec(&bad).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        owner.load_session("project__first").unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert!(writer.process_step_native(step(Some("first"))).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
