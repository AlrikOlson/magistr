use super::*;
use crate::job::JobId;

fn bytes(progress: Progress) -> String {
    let mut out = Vec::new();
    progress.write(&mut out);
    String::from_utf8(out).expect("the sequence is ASCII")
}

#[test]
fn no_loop_and_a_finished_loop_both_clear_the_bar() {
    assert_eq!(Progress::for_loop(None), Progress::Cleared);
    let mut status = LoopStatus::new(2, 4);
    status.end("aborted");
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Cleared);
}

#[test]
fn a_running_loop_is_its_iteration_as_a_percentage() {
    let status = LoopStatus::new(1, 4);
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(25));
    let status = LoopStatus::new(3, 4);
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(75));
    let status = LoopStatus::new(4, 4);
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(100));
}

#[test]
fn a_loop_with_no_ceiling_reports_nothing_rather_than_dividing_by_zero() {
    let status = LoopStatus::new(7, 0);
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(0));
}

#[test]
fn an_iteration_past_its_ceiling_is_held_at_full() {
    let status = LoopStatus::new(9, 4);
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(100));
}

#[test]
fn a_running_gate_is_indeterminate_and_a_settled_one_gives_the_fraction_back() {
    let mut status = LoopStatus::new(2, 4);
    status.start_gate("check", JobId::new(1));
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Indeterminate);
    assert!(status.settle_gate(JobId::new(1), true));
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(50));
}

#[test]
fn a_failed_gate_outranks_one_still_running() {
    let mut status = LoopStatus::new(2, 4);
    status.start_gate("check", JobId::new(1));
    status.start_gate("test", JobId::new(2));
    assert!(status.settle_gate(JobId::new(2), false));
    assert_eq!(
        Progress::for_loop(Some(&status)),
        Progress::Failed(50),
        "the failure is what the user in another window must come back for"
    );
}

#[test]
fn a_paused_loop_keeps_showing_where_it_stands() {
    let mut status = LoopStatus::new(1, 2);
    status.set_paused(true);
    assert_eq!(Progress::for_loop(Some(&status)), Progress::Running(50));
}

#[test]
fn every_state_is_one_osc_with_both_parameters() {
    assert_eq!(bytes(Progress::Cleared), "\x1b]9;4;0;0\x1b\\");
    assert_eq!(bytes(Progress::Running(42)), "\x1b]9;4;1;42\x1b\\");
    assert_eq!(bytes(Progress::Failed(75)), "\x1b]9;4;2;75\x1b\\");
    assert_eq!(bytes(Progress::Indeterminate), "\x1b]9;4;3;0\x1b\\");
}

#[test]
fn the_default_is_the_state_a_terminal_starts_in() {
    assert_eq!(Progress::default(), Progress::Cleared);
}
