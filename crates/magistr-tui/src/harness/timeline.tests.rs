use std::time::Duration;

use super::*;
use magistr_cockpit::agent::AgentEvent;

fn text(t: &str) -> Msg {
    Msg::Agent(AgentEvent::Text(t.to_owned()))
}

#[test]
fn steps_come_out_in_time_order_ties_in_insertion_order() {
    let timeline = Timeline::new()
        .at(Duration::from_millis(20), text("c"))
        .at(Duration::from_millis(5), text("a"))
        .at(Duration::from_millis(5), text("b"));
    assert_eq!(timeline.len(), 3);
    assert!(!timeline.is_empty());
    let order: Vec<Msg> = timeline.into_steps().into_iter().map(|(_, m)| m).collect();
    assert_eq!(order, vec![text("a"), text("b"), text("c")]);
}

#[tokio::test]
async fn steps_land_on_the_inbox_at_their_times() {
    let (inbox, mut rx) = Inbox::channel();
    let began = std::time::Instant::now();
    let done = Timeline::new()
        .at(Duration::from_millis(0), text("now"))
        .at(Duration::from_millis(60), text("later"))
        .play(inbox);
    assert_eq!(rx.recv().await, Some(text("now")));
    assert!(began.elapsed() < Duration::from_millis(50));
    assert_eq!(rx.recv().await, Some(text("later")));
    assert!(began.elapsed() >= Duration::from_millis(60));
    done.await.expect("the player ended");
    assert!(rx.recv().await.is_none(), "the player's sender is gone");
}

#[tokio::test]
async fn a_gone_loop_ends_the_player() {
    let (inbox, rx) = Inbox::channel();
    drop(rx);
    let done = Timeline::new()
        .at(Duration::from_millis(0), text("x"))
        .at(Duration::from_secs(10), text("never"))
        .play(inbox);
    tokio::time::timeout(Duration::from_secs(2), done)
        .await
        .expect("ended long before the second step")
        .expect("without a panic");
}
