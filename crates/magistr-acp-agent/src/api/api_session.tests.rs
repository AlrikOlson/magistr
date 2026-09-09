use super::*;

#[test]
fn a_turn_carries_the_whole_history_and_totals_accumulate() {
    let mut session = ApiSession::default();
    let first = session.begin_turn("hi");
    assert_eq!(first, vec![ChatMessage::new(Role::User, "hi")]);
    session.end_turn("hello".to_owned(), Some((10, 2)), 0.5);
    let second = session.begin_turn("more");
    assert_eq!(second.len(), 3);
    assert_eq!(second[1], ChatMessage::new(Role::Assistant, "hello"));
    session.end_turn("sure".to_owned(), Some((20, 3)), 0.25);
    assert_eq!((session.input_tokens, session.output_tokens), (30, 5));
    assert!((session.cost_usd - 0.75).abs() < 1e-9);
}

#[test]
fn a_cancelled_turn_keeps_a_partial_reply_but_drops_an_unanswered_prompt() {
    let mut session = ApiSession::default();
    session.begin_turn("count");
    session.abandon_turn("one two".to_owned());
    assert_eq!(session.history.len(), 2);
    assert_eq!(session.history[1].role, Role::Assistant);

    session.begin_turn("again");
    session.abandon_turn(String::new());
    assert_eq!(session.history.len(), 2, "the unanswered prompt is gone");
}

#[test]
fn usage_the_api_never_sent_leaves_the_totals_alone() {
    let mut session = ApiSession::default();
    session.begin_turn("hi");
    session.end_turn("hello".to_owned(), None, 0.0);
    assert_eq!((session.input_tokens, session.output_tokens), (0, 0));
}
