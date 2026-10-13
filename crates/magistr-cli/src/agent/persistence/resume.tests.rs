use super::*;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use chrono::TimeZone;

fn sample_session() -> SessionRecord {
    SessionRecord {
        session_id: "abc12345-6789-abcd-ef01-234567890abc".to_string(),
        project: "my-project".to_string(),
        workspace: "my-project".to_string(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: HistoryOutcome::Completed,
        started_at: Utc.with_ymd_and_hms(2026, 3, 10, 10, 0, 0).unwrap(),
        ended_at: Utc.with_ymd_and_hms(2026, 3, 10, 11, 30, 0).unwrap(),
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
    record.ended_at += chrono::Duration::seconds(1);
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
    record.ended_at += chrono::Duration::seconds(1);
    record.provider_session_id = None;
    super::super::storage::insert_session(&conn, &record).unwrap();
    assert!(saved_config(config(), &record.session_id).is_err());
}
