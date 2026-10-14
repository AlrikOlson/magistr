use super::*;
use crate::session::{session_outcome::SessionOutcome, session_record::SessionRecord};

#[test]
fn a_saved_row_selects_its_own_provider_and_load_id_without_an_opening_prompt() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(&root, |key| {
        (key == "XDG_DATA_HOME").then(|| directory.path().join("data").display().to_string())
    });
    let conn = crate::session::storage::open_db(
        &crate::session::storage::sessions_db_path(&inputs).unwrap(),
    )
    .unwrap();
    let now = chrono::Utc::now();
    crate::session::storage::insert_session(
        &conn,
        &SessionRecord {
            purpose: magistr_application::sessions::SessionPurpose::Conversation,
            session_id: "host-id".into(),
            name: None,
            project: root.display().to_string(),
            workspace: root.display().to_string(),
            provider: "recorded-agent".into(),
            provider_session_id: Some("provider-id".into()),
            resume_supported: true,
            started_at: now,
            updated_at: now,
            ended_at: Some(now),
            outcome: Some(SessionOutcome::Completed),
            cost_usd: None,
            turns: Some(1),
            duration_ms: 1,
            files_changed: Vec::new(),
            tags: Vec::new(),
            summary: None,
            model: Some("recorded-model".into()),
            owner: None,
        },
    )
    .unwrap();
    let config = resume_config(&inputs, "host-id".into(), agent::Backend::Acp).unwrap();
    assert_eq!(config.resume.as_deref(), Some("provider-id"));
    assert_eq!(config.provider.as_deref(), Some("recorded-agent"));
    assert_eq!(config.model.as_deref(), Some("recorded-model"));
    assert!(config.prompt.is_empty());
    assert_eq!(config.cwd, root);
    assert!(!config.continue_conversation);
}
