use crate::in_env;

use super::*;
use crate::cost::session_spend::SessionSpend;
use crate::cost::storage::load_entries;
use crate::cost::types::TokenBreakdown;

fn spend() -> SessionSpend {
    SessionSpend::new("sess-42", 1.25)
        .with_run(3, 12_000)
        .with_model("claude-opus-5")
        .with_tokens(TokenBreakdown {
            input_tokens: 900,
            output_tokens: 150,
            ..TokenBreakdown::default()
        })
}

#[test]
fn a_finished_session_lands_in_the_ledger_stamped_with_actor_and_model() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("magistr");
    std::fs::create_dir_all(&project).expect("project dir");
    let ledger = dir.path().join("costs.json");

    // MAGISTR_USER is the documented identity override and outranks git.
    if !in_env::in_env(&[(
        "MAGISTR_USER",
        Some(std::ffi::OsStr::new("dev@example.com")),
    )]) {
        return;
    }
    let wrote = record_session_at(&ledger, &project, &spend());
    assert!(wrote);

    let entries = load_entries(&ledger).expect("ledger loads");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.session_id, "sess-42");
    assert_eq!(entry.project, "magistr");
    assert_eq!(entry.cost_usd, 1.25);
    assert_eq!(entry.turns, 3);
    assert_eq!(entry.duration_ms, 12_000);
    assert_eq!(entry.tokens.total(), 1_050);
    assert_eq!(entry.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(
        entry.actor.as_deref(),
        Some("dev@example.com"),
        "the resolved identity is the actor the budget scopes match on"
    );
}

#[test]
fn an_empty_session_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    assert!(!record_session_at(
        &ledger,
        dir.path(),
        &SessionSpend::new("sess-0", 0.0)
    ));
    assert!(
        !ledger.exists(),
        "no file is created for a session with no spend"
    );
}

#[test]
fn entries_accumulate_rather_than_replace() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    assert!(record_session_at(&ledger, dir.path(), &spend()));
    assert!(record_session_at(
        &ledger,
        dir.path(),
        &SessionSpend::new("sess-43", 0.5)
    ));

    let entries = load_entries(&ledger).expect("ledger loads");
    assert_eq!(entries.len(), 2);
    let total: f64 = entries.iter().map(|e| e.cost_usd).sum();
    assert!((total - 1.75).abs() < 1e-9);
}

#[test]
fn an_unwritable_ledger_is_reported_but_not_fatal() {
    // A path whose parent is a *file* cannot be created.
    let dir = tempfile::tempdir().expect("tempdir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("write blocker");
    let ledger = blocker.join("costs.json");

    assert!(!record_session_at(&ledger, dir.path(), &spend()));
}
