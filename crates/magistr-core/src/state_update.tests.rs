use super::*;

#[test]
fn empty_batch() {
    let batch = UpdateBatch::new();
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
    assert!(batch.into_ordered().is_empty());
}

#[test]
fn batch_preserves_order() {
    let mut batch = UpdateBatch::new();
    batch.push(StateUpdate::StreamMarkdown("a".into()));
    batch.push(StateUpdate::StreamMarkdown("b".into()));
    batch.push(StateUpdate::StreamFlush);

    let ordered = batch.into_ordered();
    assert_eq!(ordered.len(), 3);
    assert!(matches!(&ordered[0], StateUpdate::StreamMarkdown(s) if s == "a"));
    assert!(matches!(&ordered[1], StateUpdate::StreamMarkdown(s) if s == "b"));
    assert!(matches!(&ordered[2], StateUpdate::StreamFlush));
}

#[test]
fn batch_moves_phase_transitions_to_end() {
    let mut batch = UpdateBatch::new();
    batch.push(StateUpdate::SetPhase(Phase::Running));
    batch.push(StateUpdate::StreamMarkdown("text".into()));
    batch.push(StateUpdate::SetWorkflowPhase(WorkflowPhase::Executing));

    let ordered = batch.into_ordered();
    assert_eq!(ordered.len(), 3);
    // Non-phase first
    assert!(matches!(&ordered[0], StateUpdate::StreamMarkdown(_)));
    // Phase transitions at end, in original relative order
    assert!(matches!(&ordered[1], StateUpdate::SetPhase(Phase::Running)));
    assert!(matches!(
        &ordered[2],
        StateUpdate::SetWorkflowPhase(WorkflowPhase::Executing)
    ));
}

#[test]
fn batch_coalesces_consecutive_flushes() {
    let mut batch = UpdateBatch::new();
    batch.push(StateUpdate::StreamFlush);
    batch.push(StateUpdate::StreamFlush);
    batch.push(StateUpdate::StreamFlush);

    let ordered = batch.into_ordered();
    assert_eq!(ordered.len(), 1);
    assert!(matches!(&ordered[0], StateUpdate::StreamFlush));
}

#[test]
fn batch_non_consecutive_flushes_preserved() {
    let mut batch = UpdateBatch::new();
    batch.push(StateUpdate::StreamFlush);
    batch.push(StateUpdate::StreamMarkdown("gap".into()));
    batch.push(StateUpdate::StreamFlush);

    let ordered = batch.into_ordered();
    assert_eq!(ordered.len(), 3);
}

#[test]
fn batch_from_updates() {
    let updates = vec![StateUpdate::StreamBeginBlock, StateUpdate::StreamFlush];
    let batch = UpdateBatch::from_updates(updates);
    assert_eq!(batch.len(), 2);
}
