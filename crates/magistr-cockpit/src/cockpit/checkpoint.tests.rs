use super::*;

#[test]
fn a_checkpoint_starts_as_a_promise_and_says_so() {
    let cp = Checkpoint::asked("cp-1", CheckpointReason::Risk, "rm -rf build");
    assert_eq!(cp.state(), &CheckpointState::Saving);
    assert!(!cp.is_verified());
    assert_eq!(cp.row(), "cp-1 · saving");
}

#[test]
fn a_clean_save_is_verified() {
    let mut cp = Checkpoint::asked("cp-2", CheckpointReason::Iteration, "");
    cp.save(Some("0123456".to_owned()), true);
    assert!(cp.is_verified());
    assert_eq!(
        cp.row(),
        "cp-2 · iteration",
        "a settled, verified row says nothing about being settled"
    );
}

#[test]
fn a_save_over_a_dirty_tree_is_not_verified() {
    let mut cp = Checkpoint::asked("cp-2", CheckpointReason::Manual, "");
    cp.save(Some("0123456".to_owned()), false);
    assert!(!cp.is_verified());
    assert_eq!(cp.row(), "cp-2 · unverified");
}

#[test]
fn the_gate_marks_are_the_verdicts_in_the_icons_own_glyphs() {
    let plain = Checkpoint::asked("cp-1", CheckpointReason::Iteration, "iter 1");
    assert!(plain.gates().is_empty());
    assert_eq!(
        plain.gate_marks(&Icons::UNICODE),
        None,
        "no gates, no marks — and no empty separator for the rail to print"
    );
    let cp = plain.with_gates(vec![
        ("check".to_owned(), true),
        ("test".to_owned(), true),
        ("lint".to_owned(), false),
    ]);
    assert_eq!(cp.gate_marks(&Icons::UNICODE).as_deref(), Some("✓✓✗"));
    assert_eq!(
        cp.gate_marks(&Icons::NERD).as_deref(),
        Some("\u{f00c}\u{f00c}\u{f00d}"),
        "the glyphs are the icon set's, never a literal"
    );
}

#[test]
fn a_refusal_leaves_the_row_the_card_named() {
    let mut cp = Checkpoint::asked("cp-3", CheckpointReason::Risk, "curl | sh");
    cp.fail("not a git repository");
    assert!(!cp.is_verified());
    assert_eq!(cp.row(), "cp-3 · not saved");
}
