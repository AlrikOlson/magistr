use super::*;

use crate::workflow::{IterationStatus, WorkflowPhase};

fn iteration(n: u32) -> IterationState {
    IterationState {
        iteration: n,
        max_iterations: 20,
        last_status: IterationStatus::Committed,
        spec_checked: 7,
        spec_total: 12,
    }
}

fn captured() -> SessionSnapshot {
    let mut session = EngineSession::new("sess-1");
    session
        .set_status(SessionStatus::InProgress)
        .expect("created -> in progress");
    session.set_phase(WorkflowPhase::Executing);
    session.record_turn(1.25, 400, 90);
    session.push_message("hello");

    SessionSnapshot::capture(&session, iteration(3), Utc::now()).with_roadmap(
        RoadmapProgress {
            total_tasks: 12,
            checked_tasks: 7,
            total_phases: 3,
            completed_phases: 1,
        },
        Some("e10c".into()),
    )
}

// ── Acceptance 1: round-trip ────────────────────────────────────────────

#[test]
fn a_snapshot_round_trips_with_status_phase_iteration_and_cost_intact() {
    let snapshot = captured();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let parsed: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.status, SessionStatus::InProgress);
    assert_eq!(parsed.phase, WorkflowPhase::Executing);
    assert_eq!(parsed.iteration, iteration(3));
    assert_eq!(parsed.cost.total_usd, 1.25);
    assert_eq!(parsed.cost.input_tokens, 400);
    assert_eq!(parsed.cost.output_tokens, 90);
    assert_eq!(parsed.cost.turns, 1);
    assert_eq!(parsed, snapshot);
}

#[test]
fn serialization_keeps_the_status_the_capture_saw_even_though_a_restore_cannot() {
    // Round-tripping a snapshot is not restoring it: the document must
    // describe the machine it was taken on, not the one reading it.
    let snapshot = captured();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let parsed: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.status, SessionStatus::InProgress);
    assert_eq!(
        parsed.restore().expect("restores").session.status(),
        SessionStatus::Awaiting
    );
}

#[test]
fn the_roadmap_cursor_and_progress_survive_the_round_trip() {
    let snapshot = captured();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let parsed: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.roadmap_cursor.as_deref(), Some("e10c"));
    assert_eq!(parsed.roadmap.checked_tasks, 7);
    assert_eq!(parsed.roadmap.total_tasks, 12);
}

#[test]
fn a_capture_declares_the_format_version() {
    assert_eq!(captured().version, SessionSnapshot::VERSION);
    assert!(captured().is_readable());
}

// ── Acceptance 3: what cannot migrate is refused explicitly ─────────────

#[test]
fn a_severable_residency_is_reported_rather_than_dropped() {
    let snapshot = captured().resident_on(vec![
        SessionResidency::AgentProcess { pid: Some(4242) },
        SessionResidency::Terminal {
            id: "term-1".into(),
        },
        SessionResidency::WorkbenchLock {
            workspace: "/srv/proj".into(),
        },
    ]);
    let restored = snapshot.restore().expect("severable residencies restore");
    assert_eq!(restored.severed.len(), 3);
    assert!(restored.lost_anything());
    let report = restored.severance_report();
    assert!(report.iter().any(|line| line.contains("agent_process")));
    assert!(report.iter().any(|line| line.contains("term-1")));
    assert!(report.iter().any(|line| line.contains("/srv/proj")));
}

#[test]
fn a_blocking_residency_refuses_the_whole_restore_and_names_itself() {
    let snapshot = captured().holding(SessionResidency::PendingPermission {
        request_id: "req-9".into(),
    });
    let err = snapshot.restore().expect_err("a pending permission blocks");
    let message = err.to_string();
    assert!(message.contains("req-9"), "names the request: {message}");
    assert!(
        message.contains("pending_permission"),
        "names the kind: {message}"
    );
}

#[test]
fn the_blocking_residency_is_named_even_among_severable_ones() {
    let snapshot = captured()
        .holding(SessionResidency::AgentProcess { pid: None })
        .holding(SessionResidency::PendingPermission {
            request_id: "req-9".into(),
        })
        .holding(SessionResidency::Terminal { id: "t".into() });
    assert_eq!(snapshot.blocking().len(), 1);
    assert!(snapshot.restore().is_err());
}

#[test]
fn an_unknown_format_version_is_refused_rather_than_read() {
    let mut snapshot = captured();
    snapshot.version = SessionSnapshot::VERSION + 1;
    assert!(!snapshot.is_readable());
    let err = snapshot
        .restore()
        .expect_err("an unknown version is refused");
    let message = err.to_string();
    assert!(message.contains("version"), "{message}");
    assert!(message.contains("sess-1"), "{message}");
}

// ── The status a restore can honestly hold ──────────────────────────────

#[test]
fn a_turn_in_flight_restores_at_the_turn_boundary_and_says_where_from() {
    let restored = captured().restore().expect("restores");
    assert_eq!(restored.session.status(), SessionStatus::Awaiting);
    assert_eq!(restored.resumed_from, Some(SessionStatus::InProgress));
}

#[test]
fn a_status_that_asserts_nothing_about_a_live_process_is_kept() {
    for status in [
        SessionStatus::Created,
        SessionStatus::Awaiting,
        SessionStatus::Completed,
        SessionStatus::Failed,
        SessionStatus::Cancelled,
    ] {
        let mut snapshot = captured();
        snapshot.status = status;
        let restored = snapshot.restore().expect("restores");
        assert_eq!(restored.session.status(), status);
        assert_eq!(restored.resumed_from, None, "no downgrade for {status}");
    }
}

// ── What the restore carries over ───────────────────────────────────────

#[test]
fn the_restored_session_keeps_its_id_phase_cost_and_creation_time() {
    let snapshot = captured();
    let restored = snapshot.restore().expect("restores");
    assert_eq!(restored.session.session_id(), "sess-1");
    assert_eq!(restored.session.phase(), WorkflowPhase::Executing);
    assert_eq!(restored.session.cost().total_usd, 1.25);
    assert_eq!(restored.session.cost().turns, 1);
    assert_eq!(restored.session.created_at(), snapshot.created_at);
    assert_eq!(restored.session.messages(), ["hello"]);
    assert_eq!(restored.iteration, iteration(3));
    assert_eq!(restored.roadmap_cursor.as_deref(), Some("e10c"));
}

#[test]
fn a_capture_declares_no_residencies_of_its_own() {
    // The engine does not know about child processes; the adapter that
    // does declares them. An empty list means nobody declared, not that
    // nothing was held.
    let session = EngineSession::new("s");
    let snapshot = SessionSnapshot::capture(&session, iteration(0), Utc::now());
    assert!(snapshot.residencies.is_empty());
}

// ── The repo the checkpoint was taken in ────

#[test]
fn a_snapshot_carries_the_repo_it_was_captured_in() {
    let snapshot = captured().in_repo(
        crate::session::RepoState::at("0123456789abcdef")
            .on_branch("master")
            .with_dirty(2),
    );
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let parsed: SessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    let repo = parsed.repo.expect("the repo crossed with the values");
    assert_eq!(repo.short_head(), "0123456");
    assert_eq!(repo.branch.as_deref(), Some("master"));
    assert!(!repo.is_clean());
}

#[test]
fn a_snapshot_written_before_the_repo_field_still_parses() {
    // New fields use serde defaults, so a document from
    // before it is read rather than refused — the version is for
    // changes that cannot be read, and this one can.
    let mut value = serde_json::to_value(captured()).expect("serialize");
    let removed = value.as_object_mut().expect("an object").remove("repo");
    assert!(removed.is_none(), "a repo-less capture writes no field");
    let parsed: SessionSnapshot = serde_json::from_value(value).expect("deserialize");
    assert!(parsed.repo.is_none());
    assert!(parsed.is_readable(), "the format version did not move");
}

#[test]
fn a_snapshot_with_no_repo_says_nothing_about_one() {
    let json = serde_json::to_string(&captured()).expect("serialize");
    assert!(!json.contains("repo"), "{json}");
}
