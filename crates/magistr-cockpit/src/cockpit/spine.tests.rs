use super::super::checkpoint_state::CheckpointState;
use super::*;

#[test]
fn an_empty_spine_holds_nothing_and_previews_nothing() {
    let spine = Spine::default();
    assert!(spine.is_empty());
    assert_eq!(spine.len(), 0);
    assert!(spine.head().is_none());
    assert!(spine.previewed().is_none());
}

#[test]
fn receipts_count_past_the_newest_checkpoint_only() {
    let mut spine = Spine::default();
    assert_eq!(
        spine.receipts_after_head(),
        None,
        "nothing is 'after' an empty ledger"
    );
    spine.note_receipt();
    spine.open(CheckpointReason::Iteration, "iter 1");
    assert_eq!(
        spine.receipts_after_head(),
        Some(0),
        "a new head starts its own count"
    );
    spine.note_receipt();
    spine.note_receipt();
    assert_eq!(spine.receipts_after_head(), Some(2));
    spine.open(CheckpointReason::Manual, "before golden update");
    assert_eq!(spine.receipts_after_head(), Some(0));
}

#[test]
fn opening_mints_the_name_the_caller_prints() {
    let mut spine = Spine::default();
    assert_eq!(spine.open(CheckpointReason::Risk, "rm -rf"), "cp-1");
    assert_eq!(spine.open(CheckpointReason::Iteration, ""), "cp-2");
    assert_eq!(spine.head().expect("a head").id(), "cp-2");
}

#[test]
fn a_boundary_row_is_named_for_its_iteration_and_wears_the_gates() {
    let mut spine = Spine::default();
    let gates = vec![
        ("check".to_owned(), true),
        ("test".to_owned(), true),
        ("lint".to_owned(), false),
    ];
    let id = spine.open_boundary(3, gates.clone());
    assert_eq!(id, "cp-1");
    let cp = spine.get(&id).expect("the row");
    assert_eq!(cp.reason(), CheckpointReason::Iteration);
    assert_eq!(cp.label(), "iter 3");
    assert_eq!(cp.gates(), gates.as_slice());
    assert_eq!(cp.state(), &CheckpointState::Saving);
    assert_eq!(
        spine.open(CheckpointReason::Manual, ""),
        "cp-2",
        "one counter names every state, whatever put it on the chain"
    );
    assert_eq!(
        spine.receipts_after_head(),
        Some(0),
        "a boundary is a new head like any other"
    );
}

#[test]
fn an_open_row_is_on_the_rail_before_the_port_answers() {
    let mut spine = Spine::default();
    let id = spine.open(CheckpointReason::Risk, "rm -rf build");
    assert_eq!(spine.len(), 1, "the card can name a row that is on screen");
    assert_eq!(
        spine.get(&id).expect("the row").state(),
        &CheckpointState::Saving
    );
}

#[test]
fn the_answer_lands_on_the_row_it_names() {
    let mut spine = Spine::default();
    spine.open(CheckpointReason::Risk, "a");
    spine.open(CheckpointReason::Risk, "b");
    spine.saved("cp-1", Some("0123456".to_owned()), true);
    assert!(spine.get("cp-1").expect("cp-1").is_verified());
    assert_eq!(
        spine.get("cp-2").expect("cp-2").state(),
        &CheckpointState::Saving,
        "cp-2 was not the one answered"
    );
}

#[test]
fn an_answer_for_a_row_that_is_not_there_changes_nothing() {
    let mut spine = Spine::default();
    spine.open(CheckpointReason::Manual, "");
    let before = spine.clone();
    spine.saved("cp-9", None, true);
    spine.failed("cp-9", "nope");
    assert_eq!(spine, before);
}

#[test]
fn a_refusal_marks_the_row_rather_than_removing_it() {
    let mut spine = Spine::default();
    spine.open(CheckpointReason::Risk, "curl | sh");
    spine.failed("cp-1", "not a git repository");
    assert_eq!(spine.len(), 1, "the card on screen still names it");
    assert!(matches!(
        spine.get("cp-1").expect("cp-1").state(),
        CheckpointState::Failed(_)
    ));
}

#[test]
fn the_mint_never_rewinds() {
    let mut spine = Spine::default();
    spine.open(CheckpointReason::Iteration, "");
    spine.open(CheckpointReason::Iteration, "");
    // A restore is not a truncation: cp-2 happened, and the next name
    // is cp-3 whatever the reader went back to look at.
    spine.preview(0);
    assert_eq!(spine.open(CheckpointReason::Manual, ""), "cp-3");
    assert_eq!(spine.len(), 3);
}

#[test]
fn previewing_selects_and_the_same_row_folds_back() {
    let mut spine = Spine::default();
    spine.open(CheckpointReason::Iteration, "");
    spine.open(CheckpointReason::Iteration, "");
    spine.preview(1);
    assert_eq!(spine.selected(), Some(1));
    assert_eq!(spine.previewed().expect("a row").id(), "cp-2");
    spine.preview(1);
    assert_eq!(spine.selected(), None, "the same click is its own undo");
}

#[test]
fn previewing_a_row_that_is_not_there_changes_nothing() {
    let mut spine = Spine::default();
    spine.open(CheckpointReason::Manual, "");
    spine.preview(4);
    assert_eq!(spine.selected(), None);
}

#[test]
fn seeding_the_counter_moves_the_ordinal_but_not_the_after_head_count() {
    let mut spine = Spine::default();
    spine.seed_receipts(239);
    assert_eq!(spine.receipts(), 239);
    spine.open(CheckpointReason::Iteration, "iter 4");
    spine.note_receipt();
    spine.note_receipt();
    assert_eq!(spine.receipts(), 241, "the card's ordinal counter");
    assert_eq!(
        spine.receipts_after_head(),
        Some(2),
        "the rail's after-head reading is untouched by the seed"
    );
}
