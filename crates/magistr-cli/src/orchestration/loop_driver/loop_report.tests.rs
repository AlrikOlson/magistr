use std::time::Duration;

use super::*;
use crate::orchestration::loop_driver::IterationMetrics;

#[test]
fn the_headline_is_unchanged_without_metrics() {
    let report = LoopReport::new(2, LoopEnd::MaxIterations(2));
    assert_eq!(
        report.to_string(),
        "loop: 2 iterations · max iterations (2) reached"
    );
    assert!(report.metrics().is_empty());
}

#[test]
fn each_iterations_metrics_print_under_the_headline() {
    let report = LoopReport::new(2, LoopEnd::MaxIterations(2))
        .with_goal(Some("native".to_owned()))
        .with_metrics(vec![
            IterationMetrics {
                turn: Duration::from_secs(600),
                gates: Duration::from_secs(90),
                judge: Some(Duration::from_secs(7)),
                tokens: Some(31_000),
            },
            IterationMetrics {
                turn: Duration::from_secs(30),
                gates: Duration::from_secs(60),
                judge: None,
                tokens: None,
            },
        ]);
    assert_eq!(
        report.to_string(),
        "loop: 2 iterations · max iterations (2) reached · goal: native\n  \
         iteration 1: turn 10m00s · gates 1m30s · judge 7.0s · 31.0k tokens in context\n  \
         iteration 2: turn 30.0s · gates 1m00s · usage not reported"
    );
    assert_eq!(report.metrics().len(), 2);
}
