use super::*;

#[test]
fn a_family_glob_classifies_every_member() {
    let rule = ModelTierRule::new("claude-opus-*", ModelTier::Frontier);
    assert!(rule.matches("claude-opus-5"));
    assert!(rule.matches("claude-opus-4-1-20250805"));
    assert!(!rule.matches("claude-haiku-4-5"));
}

#[test]
fn round_trips_through_the_roster_json() {
    let rule = ModelTierRule::new("claude-haiku-*", ModelTier::Economy);
    let json = serde_json::to_string(&rule).expect("serialize");
    assert_eq!(json, r#"{"pattern":"claude-haiku-*","tier":"economy"}"#);
    let back: ModelTierRule = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, rule);
}
