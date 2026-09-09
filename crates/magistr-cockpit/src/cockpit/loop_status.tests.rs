use std::time::Duration;

use super::*;

#[test]
fn a_fresh_status_names_the_iteration_and_counts_nothing_yet() {
    let status = LoopStatus::new(2, 10);
    assert_eq!(status.iteration(), 2);
    assert_eq!(status.max(), 10);
    assert_eq!(status.iteration_text(), "iteration 2/10");
    assert!(status.gates().is_empty());
    assert_eq!(status.elapsed(), Duration::ZERO);
    assert_eq!(status.elapsed_text(), "0s");
    assert!(!status.is_paused());
    assert_eq!(status.ended(), None);
    assert_eq!(status.gates_passed(), None, "no gate ran");
    assert_eq!(status.stage(), LoopStage::Agent, "the volume's own stage");
}

#[test]
fn the_stage_follows_the_driver_and_is_not_carried_across_iterations() {
    let mut status = LoopStatus::new(1, 3);
    status.set_stage(LoopStage::Connecting);
    assert_eq!(status.stage(), LoopStage::Connecting);
    status.set_stage(LoopStage::Judge);
    assert_eq!(status.stage(), LoopStage::Judge);
    let next = LoopStatus::new(2, 3).carrying(Some(&status));
    assert_eq!(
        next.stage(),
        LoopStage::Agent,
        "a new iteration starts over"
    );
}

#[test]
fn gates_settle_by_job_id_and_the_verdict_waits_for_the_last_one() {
    let mut status = LoopStatus::new(1, 1);
    status.start_gate("check", JobId::new(7));
    status.start_gate("test", JobId::new(8));
    assert_eq!(status.gates().len(), 2);
    assert_eq!(status.gates_passed(), None, "still running");
    assert!(status.settle_gate(JobId::new(8), true));
    assert_eq!(status.gates_passed(), None);
    assert!(
        !status.settle_gate(JobId::new(99), true),
        "not a gate's job"
    );
    assert!(status.settle_gate(JobId::new(7), false));
    assert_eq!(status.gates_passed(), Some(false));
    let texts: Vec<String> = status
        .gates()
        .iter()
        .map(|gate| gate.text(&crate::theme::Icons::UNICODE))
        .collect();
    assert_eq!(texts, ["✗ check", "✓ test"]);
}

#[test]
fn elapsed_reads_in_minutes_and_seconds() {
    let mut status = LoopStatus::new(1, 3);
    status.set_elapsed(Duration::from_secs(45));
    assert_eq!(status.elapsed_text(), "45s");
    status.set_elapsed(Duration::from_millis(83_400));
    assert_eq!(status.elapsed_text(), "1m 23s");
    status.set_elapsed(Duration::from_secs(3_725));
    assert_eq!(status.elapsed_text(), "1h 02m 05s");
}

#[test]
fn pausing_and_ending_are_recorded() {
    let mut status = LoopStatus::new(3, 3);
    status.set_paused(true);
    assert!(status.is_paused());
    status.set_paused(false);
    assert!(!status.is_paused());
    status.end("max iterations (3) reached");
    assert_eq!(status.ended(), Some("max iterations (3) reached"));
}

#[test]
fn verdicts_are_counted_across_iterations_and_the_last_one_kept() {
    let icons = &crate::theme::Icons::UNICODE;
    let mut first = LoopStatus::new(1, 3);
    assert_eq!(first.verdicts(), 0);
    assert_eq!(first.verdict(), None);
    assert_eq!(first.verdict_text(icons), None, "no judge, no LED");
    first.record_verdict(LoopVerdict {
        ok: false,
        impossible: false,
        reason: "no test yet".to_owned(),
    });
    assert_eq!(first.verdicts(), 1);
    assert_eq!(
        first.verdict_text(icons).as_deref(),
        Some("✗ judge"),
        "a not-yet wears the fail glyph"
    );

    // The next iteration's status starts from the loop's count.
    let mut second = LoopStatus::new(2, 3).carrying(Some(&first));
    assert_eq!(second.verdicts(), 1);
    assert_eq!(
        second.verdict().map(LoopVerdict::text).as_deref(),
        Some("not yet — no test yet")
    );
    second.record_verdict(LoopVerdict {
        ok: true,
        impossible: false,
        reason: "every criterion holds".to_owned(),
    });
    assert_eq!(second.verdicts(), 2);
    assert_eq!(second.verdict_text(icons).as_deref(), Some("✓ judge"));

    // Carrying nothing is the fresh status.
    assert_eq!(LoopStatus::new(1, 1).carrying(None), LoopStatus::new(1, 1));
}
