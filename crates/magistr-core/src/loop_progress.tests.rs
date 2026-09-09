use super::*;
use crate::gate::{IterationOutcome, IterationPhase};

#[test]
fn new_loop_progress() {
    let lp = LoopProgress::new(10);
    assert_eq!(lp.iteration, 1);
    assert_eq!(lp.max_iterations, 10);
    assert!(!lp.paused);
    assert!(lp.iteration_results.is_empty());
    assert!(lp.selected_iteration.is_none());
    assert_eq!(lp.spec_progress, 0.0);
}

#[test]
fn loop_progress_with_results() {
    let mut lp = LoopProgress::new(5);
    lp.iteration = 3;
    lp.iteration_results.push(IterationOutcome::Passed);
    lp.iteration_results.push(IterationOutcome::Failed);
    lp.iteration_phases.push(Some(IterationPhase::Verify));
    lp.iteration_phases.push(Some(IterationPhase::Execute));
    lp.iteration_durations.push(45.0);
    lp.iteration_durations.push(62.0);
    lp.avg_iteration_secs = 53.5;

    assert_eq!(lp.iteration_results.len(), 2);
    assert_eq!(lp.iteration_phases.len(), 2);
    assert_eq!(lp.avg_iteration_secs, 53.5);
}
