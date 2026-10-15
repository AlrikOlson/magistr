use super::*;
use magistr::config::inputs::ProjectInputs;
use magistr::session::{session_outcome::SessionOutcome, storage};

#[test]
fn unsupported_history_is_readable_and_unavailable_resume_is_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None).with_data_dir(dir.path());
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    conn.execute("INSERT INTO sessions (session_id,project,workspace,provider,purpose,resume_supported,started_at,updated_at,ended_at,outcome,duration_ms,files_changed,tags,name,summary) VALUES ('saved','p',?1,'copilot','roadmap',0,'2026-10-14T00:00:00Z','2026-10-14T00:00:00Z','2026-10-14T00:00:00Z','completed',0,'[]','[]','Review café','A saved reply')", [dir.path().display().to_string()]).unwrap();
    let mut record = storage::get_session(&conn, "saved").unwrap().unwrap();
    assert_eq!(record.outcome, Some(SessionOutcome::Completed));
    let view = saved(record.clone(), &[]);
    assert_eq!(view.name.as_deref(), Some("Review café"));
    assert_eq!(view.purpose, "roadmap");
    assert_eq!(view.summary.as_deref(), Some("A saved reply"));
    assert!(view.resume_reason.unwrap().contains("cannot resume"));
    record.summary =
        Some("## Release review\n\n- Preserve the saved draft.\n- Check the installer.".into());
    let formatted = saved(record.clone(), &[]);
    let document = formatted.document.unwrap();
    assert_eq!(document.nodes[0].tag, "h2");
    assert_eq!(document.nodes[1].tag, "ul");
    assert_eq!(formatted.summary, record.summary);
    record.resume_supported = true;
    record.provider_session_id = Some("provider".into());
    assert!(
        saved(record, &[])
            .resume_reason
            .unwrap()
            .contains("unavailable")
    );
}
