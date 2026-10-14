use super::ReasoningServer;
use crate::think::config::ThinkConfig;
use crate::think::domain::ThinkStep;
use serde_json::{Value, json};

fn quiet_config() -> ThinkConfig {
    let mut c = ThinkConfig::default();
    c.display.color_output = false;
    c.persistence.enabled = false;
    c
}

/// Build a valid new step, optionally omitting step_number (None → field
/// absent, which deserializes to the serde default 0).
fn step(step_number: Option<u32>) -> ThinkStep {
    let mut body = json!({
        "estimated_total": 10,
        "purpose": "analysis",
        "context": "ctx",
        "thought": "some reasoning",
        "outcome": "did a thing",
        "next_action": "next",
        "rationale": "because",
    });
    if let Some(n) = step_number {
        body["step_number"] = json!(n);
    }
    serde_json::from_value(body).expect("valid step json")
}

fn ok_json(server: &mut ReasoningServer, s: ThinkStep) -> Value {
    server
        .process_step_native(s)
        .expect("step should be accepted")
        .report
}

fn has_auto_warning(v: &Value) -> bool {
    v["warnings"]
        .as_array()
        .map(|w| {
            w.iter().any(|x| {
                x.as_str()
                    .is_some_and(|s| s.contains("Auto-assigned step_number"))
            })
        })
        .unwrap_or(false)
}

#[test]
fn omitted_step_number_is_auto_assigned() {
    let mut server = ReasoningServer::new(quiet_config());
    let first = ok_json(&mut server, step(None));
    assert_eq!(first["step_number"], 1, "first omitted step → 1");
    assert!(has_auto_warning(&first));

    let second = ok_json(&mut server, step(None));
    assert_eq!(second["step_number"], 2, "second omitted step → 2");
}

#[test]
fn colliding_step_number_is_auto_bumped_not_rejected() {
    let mut server = ReasoningServer::new(quiet_config());
    let first = ok_json(&mut server, step(Some(1)));
    assert_eq!(first["step_number"], 1);

    // A fresh-chat-style retry of step 1 must NOT hard-fail — it bumps.
    let bumped = ok_json(&mut server, step(Some(1)));
    assert_eq!(
        bumped["step_number"], 2,
        "collision auto-bumps to next free"
    );
    assert!(has_auto_warning(&bumped));
}

#[test]
fn explicit_unique_step_number_is_respected() {
    let mut server = ReasoningServer::new(quiet_config());
    let out = ok_json(&mut server, step(Some(1)));
    assert_eq!(out["step_number"], 1);
    // No auto-numbering warning when the caller's number is fine.
    assert!(!has_auto_warning(&out));
}
