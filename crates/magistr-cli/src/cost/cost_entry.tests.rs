use super::*;

#[test]
fn a_pre_e6_ledger_line_still_loads_with_no_attribution() {
    let json = r#"{
        "session_id": "s1",
        "project": "magistr",
        "timestamp": "2026-08-01T12:00:00Z",
        "cost_usd": 1.5,
        "turns": 3,
        "duration_ms": 9000
    }"#;
    let entry: CostEntry = serde_json::from_str(json).expect("deserialize");
    assert_eq!(entry.actor, None);
    assert_eq!(entry.model, None);
    assert_eq!(entry.session_id, "s1");
}

#[test]
fn attribution_round_trips() {
    let json = r#"{
        "session_id": "s1",
        "project": "magistr",
        "timestamp": "2026-08-01T12:00:00Z",
        "cost_usd": 1.5,
        "turns": 3,
        "duration_ms": 9000,
        "actor": "ada@example.com",
        "model": "claude-opus-5"
    }"#;
    let entry: CostEntry = serde_json::from_str(json).expect("deserialize");
    assert_eq!(entry.actor.as_deref(), Some("ada@example.com"));
    assert_eq!(entry.model.as_deref(), Some("claude-opus-5"));

    let back: CostEntry =
        serde_json::from_str(&serde_json::to_string(&entry).expect("serialize")).expect("reload");
    assert_eq!(back, entry);
}

#[test]
fn an_unattributed_entry_omits_the_fields_when_written() {
    let entry = CostEntry {
        session_id: "s1".into(),
        project: "magistr".into(),
        timestamp: chrono::Utc::now(),
        cost_usd: 0.5,
        tokens: TokenBreakdown::default(),
        turns: 1,
        duration_ms: 100,
        actor: None,
        model: None,
    };
    let json = serde_json::to_string(&entry).expect("serialize");
    assert!(!json.contains("actor"), "{json}");
    assert!(!json.contains("model"), "{json}");
}
