use super::ReasoningServer;
use crate::decision::config::DecisionConfig;
use crate::decision::domain::Note;
use serde_json::{Value, json};

fn quiet_config() -> DecisionConfig {
    let mut c = DecisionConfig::default();
    c.display.color_output = false;
    c.persistence.enabled = false;
    c
}

/// Build a valid new note, optionally omitting note_number (None → field
/// absent, which deserializes to the serde default 0).
fn note(note_number: Option<u32>) -> Note {
    let mut body = json!({
        "estimated_total": 10,
        "purpose": "analysis",
        "context": "ctx",
        "thought": "some reasoning",
        "outcome": "did a thing",
        "next_action": "next",
        "rationale": "because",
    });
    if let Some(n) = note_number {
        body["note"] = json!(n);
    }
    serde_json::from_value(body).expect("valid note json")
}

fn ok_json(server: &mut ReasoningServer, s: Note) -> Value {
    server
        .process_note_native(s)
        .expect("note should be accepted")
        .report
}

fn has_auto_warning(v: &Value) -> bool {
    v["warnings"]
        .as_array()
        .map(|w| {
            w.iter().any(|x| {
                x.as_str()
                    .is_some_and(|s| s.contains("Auto-assigned note "))
            })
        })
        .unwrap_or(false)
}

#[test]
fn omitted_note_number_is_auto_assigned() {
    let mut server = ReasoningServer::new(quiet_config());
    let first = ok_json(&mut server, note(None));
    assert_eq!(first["note"], 1, "first omitted note → 1");
    assert!(has_auto_warning(&first));

    let second = ok_json(&mut server, note(None));
    assert_eq!(second["note"], 2, "second omitted note → 2");
}

#[test]
fn colliding_note_number_is_auto_bumped_not_rejected() {
    let mut server = ReasoningServer::new(quiet_config());
    let first = ok_json(&mut server, note(Some(1)));
    assert_eq!(first["note"], 1);

    // A fresh-chat-style retry of note 1 must NOT hard-fail — it bumps.
    let bumped = ok_json(&mut server, note(Some(1)));
    assert_eq!(bumped["note"], 2, "collision auto-bumps to next free");
    assert!(has_auto_warning(&bumped));
}

#[test]
fn explicit_unique_note_number_is_respected() {
    let mut server = ReasoningServer::new(quiet_config());
    let out = ok_json(&mut server, note(Some(1)));
    assert_eq!(out["note"], 1);
    // No auto-numbering warning when the caller's number is fine.
    assert!(!has_auto_warning(&out));
}
