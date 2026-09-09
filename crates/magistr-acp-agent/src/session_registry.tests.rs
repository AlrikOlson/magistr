use super::*;

fn sid(s: &str) -> SessionId {
    SessionId::new(s)
}

#[test]
fn minted_ids_are_distinct() {
    let registry = SessionRegistry::new();
    let a = registry.mint_id();
    let b = registry.mint_id();
    assert_ne!(a, b);
    assert!(
        uuid::Uuid::parse_str(&a.to_string()).is_ok(),
        "a UUID, for the runtime's own session id: {a}"
    );
}

#[test]
fn a_session_opens_once() {
    let registry = SessionRegistry::new();
    let _rx = registry
        .open(sid("s1"), Some("m".into()))
        .expect("first open");
    let again = registry.open(sid("s1"), None);
    assert!(matches!(again, Err(AgentError::SessionExists(id)) if id == "s1"));
    assert!(registry.contains(&sid("s1")));
    assert!(!registry.contains(&sid("s2")));
}

#[test]
fn the_model_is_read_back_and_switched() {
    let registry = SessionRegistry::new();
    let _rx = registry.open(sid("s1"), Some("fast".into())).expect("open");
    assert_eq!(
        registry.model_of(&sid("s1")).expect("model"),
        Some("fast".into())
    );
    registry
        .set_model(&sid("s1"), "slow".into())
        .expect("switch");
    assert_eq!(
        registry.model_of(&sid("s1")).expect("model"),
        Some("slow".into())
    );
    assert!(matches!(
        registry.model_of(&sid("nope")),
        Err(AgentError::UnknownSession(_))
    ));
    assert!(matches!(
        registry.set_model(&sid("nope"), "x".into()),
        Err(AgentError::UnknownSession(_))
    ));
}

#[test]
fn cancel_fires_the_live_cancellation_and_arms_a_fresh_one() {
    let registry = SessionRegistry::new();
    let _rx = registry.open(sid("s1"), None).expect("open");
    let before = registry.lock()[&sid("s1")].cancel.clone();
    registry.cancel(&sid("s1")).expect("cancel");
    let after = registry.lock()[&sid("s1")].cancel.clone();
    assert!(before.is_cancelled(), "the live cancellation fired");
    assert!(!after.is_cancelled(), "the next turn starts uncancelled");
    assert!(matches!(
        registry.cancel(&sid("nope")),
        Err(AgentError::UnknownSession(_))
    ));
}

#[test]
fn admit_hands_out_the_queue_and_the_live_cancellation() {
    let registry = SessionRegistry::new();
    let _rx = registry.open(sid("s1"), None).expect("open");
    let (_, before) = registry.admit(&sid("s1")).expect("admit");
    registry.cancel(&sid("s1")).expect("cancel");
    let (_, after) = registry.admit(&sid("s1")).expect("admit again");
    assert!(
        before.is_cancelled(),
        "a turn admitted before the cancel is cancelled"
    );
    assert!(!after.is_cancelled(), "a turn admitted after it is not");
    assert!(matches!(
        registry.admit(&sid("nope")),
        Err(AgentError::UnknownSession(_))
    ));
}
