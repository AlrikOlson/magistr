use magistr_domain::policy::RiskTier;
use std::time::Instant;

use super::*;
use crate::channel::Inbox;

fn request(id: u64) -> Msg {
    Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id,
        title: "Bash".to_owned(),
        choices: Vec::new(),
        edit: false,
        risk: RiskTier::Medium,
        plan: None,
    })
}

#[test]
fn only_permission_requests_are_picked_out_in_order() {
    let arrivals = Arrivals::new();
    assert!(arrivals.is_empty());
    arrivals.record(&Msg::Agent(AgentEvent::Text("hi".to_owned())));
    arrivals.record(&request(1));
    arrivals.record(&Msg::Agent(AgentEvent::Text("more".to_owned())));
    arrivals.record(&request(2));
    assert_eq!(arrivals.len(), 4);
    let all = arrivals.instants();
    let requests = arrivals.permission_requests();
    assert_eq!(requests, [all[1], all[3]]);
}

#[tokio::test]
async fn a_stamped_inbox_records_before_the_loop_can_see_the_message() {
    let (inbox, mut rx) = Inbox::channel();
    let arrivals = Arrivals::new();
    let stamped = inbox.stamped(arrivals.clone());
    let before = Instant::now();
    assert!(stamped.send(request(1)));
    let seen = rx.recv().await.expect("the loop's channel got it too");
    assert!(matches!(
        seen,
        Msg::Agent(AgentEvent::PermissionRequest { id: 1, .. })
    ));
    let stamp = arrivals.permission_requests();
    assert_eq!(stamp.len(), 1);
    assert!(stamp[0] >= before);
    assert!(stamp[0] <= Instant::now());
}

#[test]
fn a_stamped_inbox_still_taps() {
    let (inbox, mut rx) = Inbox::channel();
    let (tap, mut tapped) = tokio::sync::mpsc::unbounded_channel();
    let arrivals = Arrivals::new();
    let stamped = inbox.tap(tap).stamped(arrivals.clone());
    assert!(stamped.send(request(7)));
    assert!(rx.try_recv().is_ok());
    assert!(tapped.try_recv().is_ok());
    assert_eq!(arrivals.len(), 1);
}
