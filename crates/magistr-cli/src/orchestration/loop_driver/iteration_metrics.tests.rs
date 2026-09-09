use std::time::Duration;

use super::*;

#[test]
fn a_full_measure_prints_every_phase_and_the_tokens() {
    let metrics = IterationMetrics {
        turn: Duration::from_millis(12_340),
        gates: Duration::from_secs(250),
        judge: Some(Duration::from_millis(8_200)),
        tokens: Some(41_234),
    };
    assert_eq!(
        metrics.to_string(),
        "turn 12.3s · gates 4m10s · judge 8.2s · 41.2k tokens in context"
    );
}

#[test]
fn an_unasked_judge_and_unreported_usage_are_said_rather_than_zeroed() {
    let metrics = IterationMetrics {
        turn: Duration::from_millis(500),
        gates: Duration::ZERO,
        judge: None,
        tokens: None,
    };
    assert_eq!(
        metrics.to_string(),
        "turn 0.5s · gates 0.0s · usage not reported"
    );
}

#[test]
fn counts_scale_by_thousands() {
    assert_eq!(count(987), "987");
    assert_eq!(count(1_000), "1.0k");
    assert_eq!(count(12_345), "12.3k");
    assert_eq!(count(2_500_000), "2.5M");
}

#[test]
fn the_clock_turns_to_minutes_at_sixty_seconds() {
    assert_eq!(clock(Duration::from_secs_f64(59.96)), "60.0s");
    assert_eq!(clock(Duration::from_secs(60)), "1m00s");
    assert_eq!(clock(Duration::from_secs(3_599)), "59m59s");
    assert_eq!(clock(Duration::from_secs(3_600)), "60m00s");
}
