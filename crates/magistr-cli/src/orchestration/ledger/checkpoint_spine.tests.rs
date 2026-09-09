use magistr_engine::workflow::{IterationState, IterationStatus};
use magistr_engine::{CheckpointReason, EngineSession, RepoState};

use super::*;

fn iteration(n: u32) -> IterationState {
    IterationState {
        iteration: n,
        max_iterations: 10,
        last_status: IterationStatus::Committed,
        spec_checked: 0,
        spec_total: 0,
    }
}

fn snapshot(n: u32) -> SessionSnapshot {
    SessionSnapshot::capture(&EngineSession::new("sess"), iteration(n), Utc::now())
}

fn spine() -> CheckpointSpine {
    CheckpointSpine::new()
}

/// Append under `id`, cloned out so the borrow ends with the call.
fn save(
    spine: &mut CheckpointSpine,
    id: &str,
    reason: CheckpointReason,
    label: &str,
    snapshot: SessionSnapshot,
) -> Checkpoint {
    spine
        .save(id, reason, label, snapshot, Utc::now())
        .expect("saves")
        .clone()
}

fn spine_next(spine: &CheckpointSpine) -> String {
    spine.next_id()
}

#[test]
fn an_empty_spine_has_no_head_and_no_rows() {
    let spine = spine();
    assert!(spine.is_empty());
    assert_eq!(spine.len(), 0);
    assert!(spine.head().is_none());
    assert!(spine.entries().is_empty());
}

#[test]
fn ids_are_minted_in_order_from_one() {
    let mut spine = spine();
    save(
        &mut spine,
        "cp-1",
        CheckpointReason::Iteration,
        "i1",
        snapshot(1),
    );
    save(
        &mut spine,
        "cp-2",
        CheckpointReason::Risk,
        "rm -rf",
        snapshot(2),
    );
    let ids: Vec<&str> = spine.entries().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["cp-1", "cp-2"]);
    assert_eq!(spine.head().expect("a head").id, "cp-2");
}

#[test]
fn the_next_id_is_what_the_next_save_mints() {
    let mut spine = spine();
    assert_eq!(spine.next_id(), "cp-1");
    let minted = spine
        .save(
            spine_next(&spine),
            CheckpointReason::Manual,
            "",
            snapshot(1),
            Utc::now(),
        )
        .expect("saves")
        .id
        .clone();
    assert_eq!(minted, "cp-1", "the promise the card printed was kept");
    assert_eq!(spine.next_id(), "cp-2");
}

#[test]
fn a_checkpoint_is_found_by_the_id_it_was_minted_with() {
    let mut spine = spine();
    save(
        &mut spine,
        "cp-1",
        CheckpointReason::Iteration,
        "i1",
        snapshot(1),
    );
    save(
        &mut spine,
        "cp-2",
        CheckpointReason::Iteration,
        "i2",
        snapshot(2),
    );
    assert_eq!(
        spine
            .get("cp-2")
            .expect("found")
            .snapshot
            .iteration
            .iteration,
        2
    );
    assert!(spine.get("cp-9").is_none());
}

#[test]
fn a_checkpoint_with_a_clean_tree_is_verified_and_names_its_commit() {
    let mut spine = spine();
    let cp = save(
        &mut spine,
        "cp-1",
        CheckpointReason::Risk,
        "rm -rf build",
        snapshot(3).in_repo(RepoState::at("0123456789ab").on_branch("master")),
    );
    assert!(cp.is_verified());
    assert_eq!(cp.head(), Some("0123456789ab"));
    assert_eq!(cp.row(), "cp-1 · risk · 0123456 · rm -rf build");
}

#[test]
fn a_checkpoint_over_a_dirty_tree_is_not_verified() {
    let mut spine = spine();
    let cp = save(
        &mut spine,
        "cp-1",
        CheckpointReason::Manual,
        "",
        snapshot(1).in_repo(RepoState::at("abcdef1234").with_dirty(4)),
    );
    assert!(
        !cp.is_verified(),
        "four files are not in the commit a fork would branch from"
    );
    assert_eq!(cp.row(), "cp-1 · manual · abcdef1", "no label, no trailer");
}

#[test]
fn a_checkpoint_outside_a_repository_is_not_verified_and_names_no_commit() {
    let mut spine = spine();
    let cp = save(
        &mut spine,
        "cp-1",
        CheckpointReason::Manual,
        "here",
        snapshot(1),
    );
    assert!(!cp.is_verified());
    assert!(cp.head().is_none());
    assert_eq!(cp.row(), "cp-1 · manual · here");
}

#[test]
fn the_same_id_is_never_two_states() {
    let mut spine = spine();
    save(
        &mut spine,
        "cp-1",
        CheckpointReason::Manual,
        "",
        snapshot(1),
    );
    let err = spine
        .save(
            "cp-1",
            CheckpointReason::Manual,
            "",
            snapshot(2),
            Utc::now(),
        )
        .expect_err("already taken");
    assert!(err.contains("cp-1"), "{err}");
    assert_eq!(spine.len(), 1, "the first state is still the only cp-1");
}

#[test]
fn the_mint_stays_ahead_of_names_it_was_told() {
    let mut spine = spine();
    // A cockpit numbering the spine hands it cp-1 and cp-2; a headless
    // caller asking for a name afterwards must not be given cp-1 again.
    save(&mut spine, "cp-1", CheckpointReason::Risk, "", snapshot(1));
    save(&mut spine, "cp-2", CheckpointReason::Risk, "", snapshot(2));
    assert_eq!(spine.next_id(), "cp-3");
}
