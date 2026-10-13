use super::*;
use magistr::config::inputs::ProjectInputs;
use magistr::session::{session_outcome::SessionOutcome, session_record::SessionRecord, storage};

#[test]
fn saved_desktop_resume_uses_the_recorded_agent_and_never_guesses_a_provider_id() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let inputs = ProjectInputs::from_lookup(&root, |key| {
        (key == "XDG_DATA_HOME").then(|| directory.path().join("data").display().to_string())
    });
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let now = "2026-10-12T12:00:00Z".parse().unwrap();
    storage::insert_session(
        &conn,
        &SessionRecord {
            session_id: "saved-host".into(),
            project: root.display().to_string(),
            workspace: root.display().to_string(),
            provider: "recorded-agent".into(),
            provider_session_id: Some("provider-thread".into()),
            resume_supported: true,
            started_at: now,
            ended_at: now,
            outcome: SessionOutcome::Completed,
            cost_usd: None,
            turns: Some(1),
            duration_ms: 1,
            files_changed: Vec::new(),
            tags: Vec::new(),
            summary: None,
            model: None,
            owner: None,
        },
    )
    .unwrap();
    let cli = Desktop::try_parse_from(["desktop", "--resume", "saved-host"]).unwrap();
    let config = cli
        .session_choices(AgentConfig::new("", &root).with_inputs(&inputs))
        .unwrap();
    assert_eq!(config.provider.as_deref(), Some("recorded-agent"));
    assert_eq!(config.resume.as_deref(), Some("provider-thread"));
    let missing = Desktop::try_parse_from(["desktop", "--resume", "missing"]).unwrap();
    assert!(
        missing
            .session_choices(AgentConfig::new("", &root).with_inputs(&inputs))
            .is_err()
    );
}
