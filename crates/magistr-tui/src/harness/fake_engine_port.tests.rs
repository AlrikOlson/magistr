use serde_json::json;

use super::*;

#[test]
fn clones_share_the_pin_log() {
    let observer = FakeEnginePort::new();
    let mut boxed: Box<dyn EnginePort> = Box::new(observer.clone());
    boxed.pin(3, true).expect("accepted");
    boxed.pin(3, false).expect("accepted");
    assert_eq!(observer.pins(), vec![(3, true), (3, false)]);
}

#[test]
fn unknown_steps_are_refused_like_the_real_engine() {
    let mut port = FakeEnginePort::new().knowing_steps_to(5);
    assert!(port.pin(5, true).is_ok());
    assert_eq!(
        port.pin(6, true),
        Err("No step #6 found in the current trace.".to_owned())
    );
    assert_eq!(port.pins(), vec![(5, true)], "the refusal left no mark");
}

#[test]
fn a_checkpoint_answers_the_scripted_report() {
    let mut port = FakeEnginePort::new().with_report(json!({"stale_branches": ["b-alt"]}));
    assert_eq!(
        port.checkpoint().expect("a report"),
        json!({"stale_branches": ["b-alt"]})
    );
}
