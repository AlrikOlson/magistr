use super::*;

fn msg(text: &str) -> SessionEvent {
    SessionEvent::MessageReceived {
        content: text.to_owned(),
    }
}

fn texts(events: &[SessionEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            SessionEvent::MessageReceived { content } => content.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn events_in_order_are_delivered_as_they_arrive() {
    let mut relay = StreamRelay::new();
    assert_eq!(relay.accept(0, 0, msg("a")).expect("accepted"), 1);
    assert_eq!(relay.accept(0, 1, msg("b")).expect("accepted"), 1);
    assert_eq!(texts(relay.delivered()), ["a", "b"]);
    assert_eq!(relay.next_seq(), 2);
}

#[test]
fn an_event_past_a_gap_waits_and_the_gap_releases_the_run() {
    let mut relay = StreamRelay::new();
    assert_eq!(relay.accept(0, 2, msg("c")).expect("accepted"), 0);
    assert_eq!(relay.accept(0, 1, msg("b")).expect("accepted"), 0);
    assert_eq!(relay.buffered(), 2);
    assert!(relay.delivered().is_empty());

    // The gap itself releases everything behind it, in sequence order.
    assert_eq!(relay.accept(0, 0, msg("a")).expect("accepted"), 3);
    assert_eq!(texts(relay.delivered()), ["a", "b", "c"]);
    assert_eq!(relay.buffered(), 0);
}

#[test]
fn a_repeat_is_dropped_rather_than_delivered_twice() {
    let mut relay = StreamRelay::new();
    relay.accept(0, 0, msg("a")).expect("accepted");
    assert_eq!(relay.accept(0, 0, msg("a")).expect("accepted"), 0);
    assert_eq!(relay.accept(0, 1, msg("b")).expect("accepted"), 1);
    assert_eq!(relay.accept(0, 1, msg("b")).expect("accepted"), 0);
    assert_eq!(texts(relay.delivered()), ["a", "b"]);
}

#[test]
fn a_repeat_still_buffered_is_dropped_too() {
    let mut relay = StreamRelay::new();
    relay.accept(0, 3, msg("d")).expect("accepted");
    assert_eq!(relay.accept(0, 3, msg("d")).expect("accepted"), 0);
    assert_eq!(relay.buffered(), 1);
}

#[test]
fn a_cursor_reads_only_what_is_new_and_never_faults_past_the_end() {
    let mut relay = StreamRelay::new();
    relay.accept(0, 0, msg("a")).expect("accepted");
    relay.accept(0, 1, msg("b")).expect("accepted");

    assert_eq!(texts(relay.since(0)), ["a", "b"]);
    assert_eq!(texts(relay.since(1)), ["b"]);
    assert!(relay.since(2).is_empty());
    assert!(relay.since(99).is_empty());
}

#[test]
fn a_stale_attempt_is_fenced_out_of_the_stream() {
    let mut relay = StreamRelay::new();
    relay.accept(0, 0, msg("a")).expect("accepted");
    relay.restart(1);

    let err = relay
        .accept(0, 1, msg("zombie"))
        .expect_err("the old holder is fenced");
    assert!(err.to_string().contains("attempt 1"), "{err}");
    assert_eq!(texts(relay.delivered()), ["a"]);
}

#[test]
fn a_future_attempt_is_fenced_too() {
    let mut relay = StreamRelay::new();
    assert!(relay.accept(7, 0, msg("early")).is_err());
}

#[test]
fn a_restart_resets_the_sequence_and_drops_the_buffer_but_keeps_the_log() {
    let mut relay = StreamRelay::new();
    relay.accept(0, 0, msg("a")).expect("accepted");
    relay.accept(0, 4, msg("orphan")).expect("accepted");
    assert_eq!(relay.buffered(), 1);

    relay.restart(1);
    assert_eq!(relay.attempt(), 1);
    assert_eq!(relay.next_seq(), 0);
    assert_eq!(relay.buffered(), 0);
    assert_eq!(texts(relay.delivered()), ["a"], "the log is append-only");

    // The new attempt appends; a reader's cursor never rewinds.
    assert_eq!(relay.accept(1, 0, msg("a-again")).expect("accepted"), 1);
    assert_eq!(texts(relay.delivered()), ["a", "a-again"]);
    assert_eq!(texts(relay.since(1)), ["a-again"]);
}

#[test]
fn a_node_that_never_closes_a_gap_is_refused_rather_than_buffered_forever() {
    let mut relay = StreamRelay::new();
    for seq in 1..=StreamRelay::MAX_PENDING as u64 {
        relay.accept(0, seq, msg("x")).expect("accepted");
    }
    assert_eq!(relay.buffered(), StreamRelay::MAX_PENDING);

    let err = relay
        .accept(0, 9_999, msg("one too many"))
        .expect_err("the buffer is bounded");
    assert!(err.to_string().contains("already waiting"), "{err}");

    // Closing the gap releases the whole run and reopens the buffer.
    let moved = relay.accept(0, 0, msg("the gap")).expect("accepted");
    assert_eq!(moved, StreamRelay::MAX_PENDING + 1);
    assert_eq!(relay.buffered(), 0);
}
