use super::*;
use crate::batch::batch_row::BatchRow;

fn row(project: &str, state: BatchState) -> BatchRow {
    BatchRow {
        project: project.to_owned(),
        state,
        job_id: format!("b1:{project}"),
    }
}

#[test]
fn an_empty_batch_is_settled_and_counts_nothing() {
    let status = BatchStatus::from_rows("b1", vec![]);
    assert_eq!(status.total, 0);
    assert!(status.settled, "nothing outstanding is settled");
    assert!(!status.has_losses());
}

#[test]
fn the_counts_are_folded_from_the_rows_and_sum_to_the_total() {
    let status = BatchStatus::from_rows(
        "b1",
        vec![
            row("a", BatchState::Succeeded),
            row("b", BatchState::Failed),
            row("c", BatchState::Skipped),
            row("d", BatchState::Running),
            row("e", BatchState::Queued),
            row("f", BatchState::Waiting),
        ],
    );
    assert_eq!(status.total, 6);
    assert_eq!(
        status.waiting
            + status.queued
            + status.running
            + status.succeeded
            + status.failed
            + status.skipped,
        status.total,
        "every row is counted exactly once"
    );
    assert!(!status.settled, "two rows are still moving");
    assert!(status.has_losses());
}

#[test]
fn a_batch_that_only_succeeded_is_settled_without_losses() {
    let status = BatchStatus::from_rows(
        "b1",
        vec![
            row("a", BatchState::Succeeded),
            row("b", BatchState::Succeeded),
        ],
    );
    assert!(status.settled);
    assert!(!status.has_losses());
}

#[test]
fn a_batch_that_lost_projects_still_settles() {
    // Settled is "nothing is moving", not "nothing went wrong" — a run
    // whose dependents were skipped is over, and the operator needs to
    // be told it is over.
    let status = BatchStatus::from_rows(
        "b1",
        vec![row("a", BatchState::Failed), row("b", BatchState::Skipped)],
    );
    assert!(status.settled);
    assert!(status.has_losses());
}

#[test]
fn a_status_round_trips_over_the_wire() {
    let status = BatchStatus::from_rows("b1", vec![row("a", BatchState::Running)]);
    let json = serde_json::to_string(&status).expect("serialize");
    let parsed: BatchStatus = serde_json::from_str(&json).expect("parse");
    assert_eq!(parsed, status);
}
