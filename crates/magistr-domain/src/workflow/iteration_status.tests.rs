use alloc::string::ToString;

use super::*;

#[test]
fn iteration_status_display() {
    assert_eq!(IterationStatus::None.to_string(), "none");
    assert_eq!(IterationStatus::Committed.to_string(), "committed");
    assert_eq!(IterationStatus::NoCommit.to_string(), "no_commit");
    assert_eq!(IterationStatus::Failed.to_string(), "failed");
}
