use super::*;

fn history() -> ThinkHistory {
    ThinkHistory {
        steps: Vec::new(),
        branches: None,
        completed: false,
        session_id: None,
        created_at: None,
        updated_at: None,
        metadata: None,
    }
}

#[test]
fn default_read_distinguishes_absence_empty_schema_decode_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let config = PersistenceConfig {
        enabled: true,
        data_dir: dir.path().to_owned(),
    };
    let owner = Persistence::for_project(&config, "project");
    assert_eq!(owner.load_default().unwrap().source, ReadSource::Missing);
    owner.save_default(&history());
    assert!(matches!(
        owner.load_default().unwrap().source,
        ReadSource::File(_)
    ));
    let path = owner.default_store_path().unwrap();
    let good = fs::read(&path).unwrap();
    let mut bad: serde_json::Value = serde_json::from_slice(&good).unwrap();
    bad["schema_version"] = serde_json::json!(999);
    fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert_eq!(
        owner.load_default().unwrap_err().kind,
        ReadErrorKind::Schema
    );
    fs::write(&path, "{").unwrap();
    assert_eq!(
        owner.load_default().unwrap_err().kind,
        ReadErrorKind::Decode
    );
    let mut wrong: serde_json::Value = serde_json::from_slice(&good).unwrap();
    wrong["history"]["metadata"] = serde_json::json!({"project_id":"elsewhere"});
    fs::write(&path, serde_json::to_vec(&wrong).unwrap()).unwrap();
    assert_eq!(
        owner.load_default().unwrap_err().kind,
        ReadErrorKind::Identity
    );
    fs::write(&path, good).unwrap();
    assert!(owner.load_default().is_ok());
}

#[test]
fn disabled_and_invalid_project_identity_never_guess_an_archive_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = PersistenceConfig {
        enabled: false,
        data_dir: dir.path().to_owned(),
    };
    let owner = Persistence::for_project(&config, "project");
    assert_eq!(owner.load_default().unwrap().source, ReadSource::Memory);
    config.enabled = true;
    let invalid = Persistence::for_project(&config, "../outside");
    assert_eq!(
        invalid.load_default().unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert!(!dir.path().join("think").exists());
}

#[test]
fn named_session_requires_its_exact_project_and_saved_scope() {
    let dir = tempfile::tempdir().unwrap();
    let config = PersistenceConfig {
        enabled: true,
        data_dir: dir.path().to_owned(),
    };
    let owner = Persistence::for_project(&config, "project");
    assert_eq!(
        owner.load_session("other__review").unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert_eq!(
        owner.load_session("project__review").unwrap().source,
        ReadSource::Missing
    );
    let mut saved = history();
    saved.session_id = Some("project__review".into());
    owner.save_session("project__review", &saved);
    let path = owner.session_store_path("project__review").unwrap();
    let original = fs::read(&path).unwrap();
    assert_eq!(
        serde_json::to_value(owner.load_session("project__review").unwrap().value).unwrap(),
        serde_json::to_value(Some(saved)).unwrap()
    );
    let mut wrong: serde_json::Value = serde_json::from_slice(&original).unwrap();
    wrong["history"]["session_id"] = serde_json::Value::Null;
    let wrong_bytes = serde_json::to_vec(&wrong).unwrap();
    fs::write(&path, &wrong_bytes).unwrap();
    assert_eq!(
        owner.load_session("project__review").unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert!(owner.load_sessions().is_err());
    assert_eq!(fs::read(path).unwrap(), wrong_bytes);
}
