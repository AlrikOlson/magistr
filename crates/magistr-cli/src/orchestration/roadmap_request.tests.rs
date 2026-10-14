use super::*;

#[test]
fn roadmap_requests_create_a_distinct_history_without_changing_provider_choices() {
    let base = AgentConfig::new("old opening", "/tmp")
        .provider("codex")
        .model("chosen")
        .resume("old-session");
    let first = config(&base);
    let second = config(&base);
    assert_eq!(first.provider, base.provider);
    assert_eq!(first.model, base.model);
    assert!(first.prompt.is_empty() && first.resume.is_none());
    assert!(!first.continue_conversation);
    assert_eq!(first.history.purpose(), SessionPurpose::Roadmap);
    assert_eq!(base.history.purpose(), SessionPurpose::Conversation);
    assert_ne!(first.history.id(), base.history.id());
    assert_ne!(first.history.id(), second.history.id());
}
