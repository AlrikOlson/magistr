use super::*;

#[test]
fn legacy_provider_id_requires_an_explicit_scoped_binding() {
    use magistr_domain::identity::{ProjectId, RunId, SessionId as DomainSessionId};

    let project = ProjectId::new("project").expect("fixture");
    let run = RunId::new(project, "run").expect("fixture");
    let local = DomainSessionId::new(run, "local-session").expect("fixture");
    let legacy = SessionId::new("provider:remote-session");
    let binding = legacy
        .bind_provider_id("codex", local.clone())
        .expect("binding");
    assert_eq!(binding.session(), &local);
    assert_eq!(binding.provider_session().provider(), "codex");
    assert_eq!(binding.provider_session().as_str(), legacy.as_str());
    assert!(legacy.bind_provider_id("", local).is_err());
}

#[test]
fn new_and_as_str() {
    let id = SessionId::new("sess-123");
    assert_eq!(id.as_str(), "sess-123");
}

#[test]
fn display() {
    let id = SessionId::new("abc");
    assert_eq!(format!("{id}"), "abc");
}

#[test]
fn clone_is_equal() {
    let id = SessionId::new("x");
    let id2 = id.clone();
    assert_eq!(id, id2);
}

#[test]
fn hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SessionId::new("a"));
    set.insert(SessionId::new("a"));
    assert_eq!(set.len(), 1);
}
