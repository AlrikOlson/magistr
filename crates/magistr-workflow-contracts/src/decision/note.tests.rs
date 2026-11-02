use super::Note;
use serde_json::json;

#[test]
fn recorded_identity_round_trips_without_deriving_it_from_prose_or_number() {
    let mut note: Note = serde_json::from_value(json!({
        "note": 3, "purpose": "analysis", "thought": "same words",
        "timestamp": "2026-10-14T00:00:00Z", "record_id": "authored-id"
    }))
    .unwrap();
    note.note = 900;
    let copy: Note = serde_json::from_value(serde_json::to_value(&note).unwrap()).unwrap();
    assert_eq!(copy.record_id.as_deref(), Some("authored-id"));
    assert_eq!(copy.note, 900);
}

#[test]
fn unrecorded_input_has_no_manufactured_identity() {
    let note: Note = serde_json::from_value(json!({
        "purpose": "analysis", "thought": "same words"
    }))
    .unwrap();
    assert_eq!(note.record_id, None);
}
