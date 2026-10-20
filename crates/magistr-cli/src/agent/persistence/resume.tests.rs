use super::*;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use chrono::TimeZone;

fn sample_session() -> SessionRecord {
    SessionRecord {
        purpose: magistr_application::sessions::SessionPurpose::Conversation,
        session_id: "abc12345-6789-abcd-ef01-234567890abc".to_string(),
        name: None,
        project: "my-project".to_string(),
        workspace: "my-project".to_string(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: Some(HistoryOutcome::Completed),
        started_at: Utc.with_ymd_and_hms(2026, 3, 10, 10, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2026, 3, 10, 11, 30, 0).unwrap(),
        ended_at: Some(Utc.with_ymd_and_hms(2026, 3, 10, 11, 30, 0).unwrap()),
        cost_usd: Some(2.50),
        turns: Some(15),
        duration_ms: 5_400_000,
        files_changed: vec!["src/main.rs".into(), "src/lib.rs".into()],
        tags: vec!["auth-refactor".into()],
        summary: Some("Refactored auth module".into()),
        model: None,
        owner: None,
    }
}

#[test]
fn from_session_has_no_changes() {
    let ctx = ResumeContext::from_session(sample_session());
    assert!(!ctx.has_changes_since());
    assert!(ctx.commits_since.is_empty());
    assert!(ctx.files_changed_since.is_empty());
    assert!(!ctx.has_dirty_working_tree);
}

#[test]
fn has_changes_when_commits_exist() {
    let mut ctx = ResumeContext::from_session(sample_session());
    ctx.commits_since.push(CommitInfo {
        sha: "abc1234".into(),
        subject: "fix: typo".into(),
        date: "2026-03-10".into(),
    });
    assert!(ctx.has_changes_since());
}

#[test]
fn has_changes_when_dirty_tree() {
    let mut ctx = ResumeContext::from_session(sample_session());
    ctx.has_dirty_working_tree = true;
    assert!(ctx.has_changes_since());
}

#[test]
fn has_changes_when_files_changed() {
    let mut ctx = ResumeContext::from_session(sample_session());
    ctx.files_changed_since.push("README.md".into());
    assert!(ctx.has_changes_since());
}

#[test]
fn summary_contains_session_info() {
    let ctx = ResumeContext::from_session(sample_session());
    let text = ctx.summary();
    assert!(
        text.contains("abc12345"),
        "Should contain truncated session ID"
    );
    assert!(text.contains("my-project"), "Should contain project name");
    assert!(text.contains("$2.50"), "Should contain cost");
    assert!(text.contains("15"), "Should contain turn count");
    assert!(text.contains("Files changed: 2"), "Should show file count");
    assert!(
        text.contains("Refactored auth module"),
        "Should show summary"
    );
    assert!(text.contains("auth-refactor"), "Should show tags");
}

#[test]
fn summary_shows_commits_since() {
    let mut ctx = ResumeContext::from_session(sample_session());
    ctx.commits_since.push(CommitInfo {
        sha: "abc1234".into(),
        subject: "fix: typo in readme".into(),
        date: "2026-03-10".into(),
    });
    ctx.commits_since.push(CommitInfo {
        sha: "def5678".into(),
        subject: "feat: add logging".into(),
        date: "2026-03-10".into(),
    });
    let text = ctx.summary();
    assert!(text.contains("2 new commit(s)"));
    assert!(text.contains("abc1234 fix: typo in readme"));
    assert!(text.contains("def5678 feat: add logging"));
}

#[test]
fn summary_truncates_long_commit_list() {
    let mut ctx = ResumeContext::from_session(sample_session());
    for i in 0..8 {
        ctx.commits_since.push(CommitInfo {
            sha: format!("sha{i:04}"),
            subject: format!("commit {i}"),
            date: "2026-03-10".into(),
        });
    }
    let text = ctx.summary();
    assert!(text.contains("8 new commit(s)"));
    assert!(text.contains("... and 3 more"));
}

#[test]
fn summary_shows_dirty_tree_warning() {
    let mut ctx = ResumeContext::from_session(sample_session());
    ctx.has_dirty_working_tree = true;
    let text = ctx.summary();
    assert!(text.contains("Uncommitted changes"));
}

#[test]
fn summary_no_changes_section_when_clean() {
    let ctx = ResumeContext::from_session(sample_session());
    let text = ctx.summary();
    assert!(
        !text.contains("Changes since"),
        "Should not show changes section when no changes"
    );
}

#[test]
fn truncate_id_short() {
    assert_eq!(truncate_id("abc"), "abc");
}

#[test]
fn truncate_id_long() {
    assert_eq!(truncate_id("abc12345-6789"), "abc12345");
}

#[test]
fn build_in_non_git_dir_returns_empty_git_info() {
    let dir = tempfile::TempDir::new().unwrap();
    let ctx = ResumeContext::build(sample_session(), dir.path());
    assert!(ctx.commits_since.is_empty());
    assert!(ctx.files_changed_since.is_empty());
    // has_dirty_working_tree may be false since git status fails
}

#[test]
fn commit_info_fields() {
    let info = CommitInfo {
        sha: "abc1234".into(),
        subject: "fix: something".into(),
        date: "2026-03-10 10:00:00 +0000".into(),
    };
    assert_eq!(info.sha, "abc1234");
    assert_eq!(info.subject, "fix: something");
}

#[test]
fn saved_resume_refuses_wrong_agent_working_copy_and_missing_load_capability() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(&root, |key| {
        (key == "XDG_DATA_HOME").then(|| directory.path().join("data").display().to_string())
    });
    let conn =
        super::super::storage::open_db(&super::super::storage::sessions_db_path(&inputs).unwrap())
            .unwrap();
    let mut record = sample_session();
    record.workspace = root.display().to_string();
    record.provider = "recorded".into();
    record.resume_supported = false;
    super::super::storage::insert_session(&conn, &record).unwrap();
    let config = || crate::agent::AgentConfig::new("", &root).with_inputs(&inputs);
    assert!(saved_config(config(), &record.session_id).is_err());
    record.updated_at += chrono::Duration::seconds(1);
    record.resume_supported = true;
    super::super::storage::insert_session(&conn, &record).unwrap();
    assert!(saved_config(config().provider("another-agent"), &record.session_id).is_err());
    let other = tempfile::tempdir().unwrap();
    assert!(
        saved_config(
            crate::agent::AgentConfig::new("", other.path()).with_inputs(&inputs),
            &record.session_id
        )
        .is_err()
    );
    record.updated_at += chrono::Duration::seconds(1);
    record.provider_session_id = None;
    super::super::storage::insert_session(&conn, &record).unwrap();
    assert!(saved_config(config(), &record.session_id).is_err());
}

#[test]
fn saved_planning_restores_exact_identity_model_provider_and_purpose() {
    use magistr_application::sessions::SessionPurpose;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(&root, |key| {
        (key == "XDG_DATA_HOME").then(|| directory.path().join("data").display().to_string())
    });
    let path = super::super::storage::sessions_db_path(&inputs).unwrap();
    let conn = super::super::storage::open_db(&path).unwrap();
    let mut record = sample_session();
    record.workspace = root.display().to_string();
    record.purpose = SessionPurpose::Roadmap;
    record.model = Some("saved-model".into());
    super::super::storage::insert_session(&conn, &record).unwrap();
    drop(conn);
    let base = crate::agent::AgentConfig::new("next request", &root)
        .with_inputs(&inputs)
        .model("unrelated-default");
    let unrelated = base.history.id();
    let restored = saved_config(base, &record.session_id).unwrap();
    assert_ne!(restored.history.id(), unrelated);
    assert_eq!(restored.history.id(), record.session_id);
    assert_eq!(restored.history.purpose(), SessionPurpose::Roadmap);
    assert_eq!(restored.resume, record.provider_session_id);
    assert_eq!(restored.model, record.model);
    assert_eq!(restored.provider.as_deref(), Some(record.provider.as_str()));
    assert_eq!(restored.prompt, "next request");
}

fn history_fixture() -> (
    tempfile::TempDir,
    crate::config::inputs::ProjectInputs,
    rusqlite::Connection,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(&root, |key| {
        (key == "XDG_DATA_HOME").then(|| root.join("data").display().to_string())
    });
    let db = super::super::storage::sessions_db_path(&inputs).unwrap();
    let conn = super::super::storage::open_db(&db).unwrap();
    (directory, inputs, conn)
}

#[test]
fn continue_uses_the_latest_checkpoint_in_this_working_copy_and_keeps_its_identity() {
    let (_directory, inputs, conn) = history_fixture();
    let mut record = sample_session();
    record.workspace = inputs.project_root().display().to_string();
    record.session_id = "older-checkpoint".into();
    super::super::storage::insert_session(&conn, &record).unwrap();
    record.session_id = "latest-checkpoint".into();
    record.started_at -= chrono::Duration::days(1);
    record.updated_at += chrono::Duration::hours(1);
    record.provider_session_id = Some("retained-provider-context".into());
    record.model = Some("recorded-model".into());
    record.purpose = magistr_application::sessions::SessionPurpose::Roadmap;
    super::super::storage::insert_session(&conn, &record).unwrap();
    let mut other = record.clone();
    other.session_id = "other-working-copy".into();
    other.workspace.push_str("/other");
    other.updated_at += chrono::Duration::hours(1);
    super::super::storage::insert_session(&conn, &other).unwrap();
    let config =
        crate::agent::AgentConfig::new("follow up", inputs.project_root()).with_inputs(&inputs);
    let resumed = config.with_run_args(None, None, true).unwrap();
    assert_eq!(resumed.history.id(), record.session_id);
    assert_eq!(resumed.history.purpose(), record.purpose);
    assert_eq!(resumed.resume, record.provider_session_id);
    assert_eq!(resumed.provider.as_deref(), Some(record.provider.as_str()));
    assert_eq!(resumed.model, record.model);
    assert_eq!(resumed.prompt, "follow up");
}

#[test]
fn continue_refuses_an_unresumable_latest_record_without_selecting_an_older_one() {
    let (_directory, inputs, conn) = history_fixture();
    let mut record = sample_session();
    record.workspace = inputs.project_root().display().to_string();
    super::super::storage::insert_session(&conn, &record).unwrap();
    record.session_id = "latest-unavailable".into();
    record.updated_at += chrono::Duration::hours(1);
    record.resume_supported = false;
    super::super::storage::insert_session(&conn, &record).unwrap();
    let config = || crate::agent::AgentConfig::new("", inputs.project_root()).with_inputs(&inputs);
    let error = config().with_run_args(None, None, true).err().unwrap();
    assert!(error.to_string().contains("cannot resume"), "{error}");
    record.resume_supported = true;
    record.updated_at += chrono::Duration::seconds(1);
    super::super::storage::insert_session(&conn, &record).unwrap();
    assert!(
        config()
            .provider("another-agent")
            .with_run_args(None, None, true)
            .is_err()
    );
    let resumed = config()
        .with_run_args(Some("chosen-model"), None, true)
        .unwrap();
    assert_eq!(resumed.model.as_deref(), Some("chosen-model"));
    assert_eq!(resumed.history.id(), record.session_id);
    assert!(
        config()
            .with_run_args(None, Some(&record.session_id), true)
            .is_err()
    );
}

#[test]
fn continue_without_history_does_not_create_a_database_or_a_new_conversation() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(directory.path(), |key| {
        (key == "XDG_DATA_HOME").then(|| directory.path().join("data").display().to_string())
    });
    let path = super::super::storage::sessions_db_path(&inputs).unwrap();
    let config = || crate::agent::AgentConfig::new("", directory.path()).with_inputs(&inputs);
    let error = config().with_run_args(None, None, true).err().unwrap();
    assert!(
        error.to_string().contains("No saved conversation"),
        "{error}"
    );
    assert!(!path.exists());
    let conn = super::super::storage::open_db(&path).unwrap();
    let error = config().with_run_args(None, None, true).err().unwrap();
    assert!(
        error.to_string().contains("No saved conversation"),
        "{error}"
    );
    assert!(
        super::super::storage::query_sessions(
            &conn,
            &super::super::session_filter::SessionFilter::new()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn continue_does_not_repair_or_replace_unreadable_history() {
    let (_directory, inputs, conn) = history_fixture();
    conn.pragma_update(None, "user_version", 999).unwrap();
    let config = crate::agent::AgentConfig::new("", inputs.project_root()).with_inputs(&inputs);
    let error = config.with_run_args(None, None, true).err().unwrap();
    assert!(error.to_string().contains("cannot be read"), "{error}");
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 999);
}
