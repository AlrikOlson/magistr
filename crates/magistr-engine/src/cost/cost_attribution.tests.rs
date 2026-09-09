use super::*;

#[test]
fn empty_attribution_still_labels_every_dimension() {
    let a = CostAttribution::default();
    assert_eq!(a.actor_label(), "unattributed");
    assert_eq!(a.model_label(), "unknown");
    assert_eq!(a.provider_label(), "anthropic");
    assert_eq!(a.project_label(), "unknown");
    assert_eq!(a.scope_label(), "unbound");
}

#[test]
fn builders_set_every_dimension() {
    let a = CostAttribution::default()
        .with_actor("dev@example.com")
        .with_model("claude-opus-5")
        .with_provider("openai")
        .with_project("magistr")
        .with_scope(BudgetScope::Team);
    assert_eq!(a.actor_label(), "dev@example.com");
    assert_eq!(a.model_label(), "claude-opus-5");
    assert_eq!(a.provider_label(), "openai");
    assert_eq!(a.project_label(), "magistr");
    assert_eq!(a.scope_label(), "team");
}

#[test]
fn blank_values_fall_back_rather_than_emitting_an_empty_label() {
    let a = CostAttribution::default().with_actor("   ").with_model("");
    assert_eq!(a.actor_label(), "unattributed");
    assert_eq!(a.model_label(), "unknown");
}

#[test]
fn absent_fields_are_omitted_from_the_wire_form() {
    let json = serde_json::to_string(&CostAttribution::default()).expect("serializes");
    assert_eq!(json, "{}");
}

#[test]
fn round_trips_through_json() {
    let a = CostAttribution::default()
        .with_actor("dev")
        .with_scope(BudgetScope::Session);
    let json = serde_json::to_string(&a).expect("serializes");
    let back: CostAttribution = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, a);
}
