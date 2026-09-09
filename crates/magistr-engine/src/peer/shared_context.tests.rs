use super::*;
use serde_json::{Value, json};

#[path = "peer_briefing.tests.rs"]
mod packet;

#[path = "shared_context_replay.tests.rs"]
mod replay;

fn observation() -> ReconciliationObservation {
    ReconciliationObservation {
        now_unix_ms: 1,
        total_tokens: 0,
        worker_attempts: 0,
    }
}

fn evidence() -> PeerEvidence {
    PeerEvidence {
        roadmap: "roadmap revision 42: build then review".into(),
        code: "git abc: code evidence".into(),
        routing_history: "history 0: no prior workers".into(),
    }
}

fn briefing() -> PeerBriefing {
    let mut plan = body();
    plan.context = evidence().context(plan.context.constraints);
    PeerBriefing::new(contract(), plan, evidence()).unwrap()
}

fn authority() -> TaskAuthority {
    TaskAuthority::with_briefing(briefing(), observation()).unwrap()
}

fn send(
    value: &mut TaskAuthority,
    peer: usize,
    request: Value,
) -> Result<AuthorityOutcome, AgreementError> {
    value.receive_peer(
        value.reconciliation().observation(),
        &contract().peers[peer],
        &serde_json::to_vec(&json!({"schema_version":1,"request":request})).unwrap(),
    )
}

fn deliver(value: &mut TaskAuthority, peer: usize) {
    value
        .apply(
            observation(),
            AuthorityAction::DeliverBriefing {
                peer: contract().peers[peer].clone(),
                briefing: briefing().revision().reference(),
            },
        )
        .unwrap();
}

fn acknowledge(value: &mut TaskAuthority, peer: usize) -> Result<AuthorityOutcome, AgreementError> {
    send(
        value,
        peer,
        json!({"kind":"acknowledge_briefing","briefing":briefing().revision().reference()}),
    )
}

fn propose(value: &mut TaskAuthority, peer: usize) -> Result<AuthorityOutcome, AgreementError> {
    send(
        value,
        peer,
        json!({"kind":"propose",
        "base":value.reconciliation().revision().reference(),
        "body":value.reconciliation().revision().body()}),
    )
}

fn ready(value: &mut TaskAuthority) {
    for peer in 0..2 {
        deliver(value, peer);
        acknowledge(value, peer).unwrap();
    }
}

#[test]
fn both_peers_get_identical_content_bound_briefing() {
    let mut value = authority();
    let a = value.briefing_for(&contract().peers[0]).unwrap().to_vec();
    assert_eq!(a, value.briefing_for(&contract().peers[1]).unwrap());
    let restored = PeerBriefing::from_json(&a, &contract()).unwrap();
    assert_eq!(
        restored.revision().reference(),
        value.reconciliation().revision().reference()
    );
    assert_eq!(restored.evidence(), &evidence());
    assert_eq!(restored.to_json().unwrap(), a);
    let mut stranger = contract().peers[0].clone();
    stranger.session_id.push('x');
    assert!(value.briefing_for(&stranger).is_err());
    ready(&mut value);
    propose(&mut value, 0).unwrap();
    assert!(value.reconciliation().proposals().is_none());
    assert_eq!(a, value.briefing_for(&contract().peers[1]).unwrap());
}

#[test]
fn neither_host_nor_peer_can_propose_before_both_acknowledge() {
    let mut value = authority();
    for stage in 0..4 {
        assert!(propose(&mut value, 0).is_err(), "stage {stage}");
        assert!(propose(&mut value, 1).is_err(), "stage {stage}");
        let revision = value.reconciliation().revision();
        let action = AuthorityAction::Protocol {
            action: ReconciliationAction::Propose {
                peer: contract().peers[0].clone(),
                base: revision.reference(),
                body: Box::new(revision.body().clone()),
            },
        };
        assert!(
            value.apply(observation(), action).is_err(),
            "host stage {stage}"
        );
        match stage {
            0 => deliver(&mut value, 0),
            1 => {
                acknowledge(&mut value, 0).unwrap();
            }
            2 => deliver(&mut value, 1),
            _ => {
                acknowledge(&mut value, 1).unwrap();
            }
        }
    }
    propose(&mut value, 0).unwrap();
    assert!(value.reconciliation().proposals().is_none());
    propose(&mut value, 1).unwrap();
    assert!(value.reconciliation().proposals().is_some());
    assert_eq!(
        value.reconciliation().state(),
        ReconciliationState::Collecting
    );
}

#[test]
fn acknowledgements_require_delivery_exact_identity_and_initial_reference() {
    let mut value = authority();
    assert!(acknowledge(&mut value, 0).is_err());
    assert!(
        send(
            &mut value,
            0,
            json!({"kind":"deliver_briefing",
        "briefing":briefing().revision().reference()})
        )
        .is_err()
    );
    deliver(&mut value, 0);
    let mut stale = briefing().revision().reference();
    stale.digest.push('x');
    assert!(
        send(
            &mut value,
            0,
            json!({"kind":"acknowledge_briefing","briefing":stale})
        )
        .is_err()
    );
    assert!(
        send(
            &mut value,
            0,
            json!({"kind":"acknowledge_briefing",
        "briefing":briefing().revision().reference(),"peer":contract().peers[1]})
        )
        .is_err()
    );
    for _ in 0..2 {
        deliver(&mut value, 0);
        acknowledge(&mut value, 0).unwrap();
        assert!(propose(&mut value, 0).is_err());
    }
    deliver(&mut value, 1);
    acknowledge(&mut value, 1).unwrap();
    propose(&mut value, 0).unwrap();
}
