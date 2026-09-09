use super::*;

#[test]
fn the_three_shapes_round_trip() {
    for (verdict, json) in [
        (
            Verdict::met("gates green"),
            r#"{"ok":true,"reason":"gates green"}"#,
        ),
        (
            Verdict::not_yet("tests missing"),
            r#"{"ok":false,"reason":"tests missing"}"#,
        ),
        (
            Verdict::impossible("criterion contradicts the code"),
            r#"{"ok":false,"reason":"criterion contradicts the code","impossible":true}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&verdict).expect("json"), json);
        assert_eq!(
            serde_json::from_str::<Verdict>(json).expect("parse"),
            verdict
        );
    }
}

#[test]
fn an_unknown_field_is_ignored_and_reason_defaults() {
    let parsed: Verdict = serde_json::from_str(r#"{"ok":true,"confidence":0.9}"#).expect("parse");
    assert_eq!(parsed, Verdict::met(""));
}

#[test]
fn the_schema_is_valid_json_requiring_ok_and_reason_and_nothing_else() {
    let schema: serde_json::Value = serde_json::from_str(Verdict::SCHEMA).expect("schema json");
    assert_eq!(schema["required"], serde_json::json!(["ok", "reason"]));
    assert_eq!(schema["additionalProperties"], false);
    let props = schema["properties"].as_object().expect("properties");
    let mut names: Vec<&str> = props.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["impossible", "ok", "reason"]);
}

#[test]
fn from_text_reads_bare_and_fenced_json() {
    let bare = Verdict::from_text(r#"{"ok": true, "reason": "done"}"#);
    assert_eq!(bare, Verdict::met("done"));
    let fenced = Verdict::from_text(
        "Here is my verdict:\n```json\n{\"ok\": false, \"reason\": \"no tests\"}\n```\n",
    );
    assert_eq!(fenced, Verdict::not_yet("no tests"));
    let impossible = Verdict::from_text(r#"{"ok":false,"impossible":true,"reason":"nope"}"#);
    assert_eq!(impossible, Verdict::impossible("nope"));
}

#[test]
fn text_that_is_not_a_verdict_is_not_yet_with_the_text_as_the_reason() {
    let chatty = Verdict::from_text("I think it looks good, ship it!");
    assert!(!chatty.ok);
    assert!(!chatty.impossible);
    assert!(chatty.reason.contains("ship it"), "{}", chatty.reason);
    let broken = Verdict::from_text("{\"ok\": tru");
    assert!(!broken.ok && !broken.impossible);
}

#[test]
fn met_and_impossible_together_is_read_as_not_ok() {
    let contradictory = Verdict::from_text(r#"{"ok":true,"impossible":true,"reason":"?"}"#);
    assert!(!contradictory.ok);
    assert!(contradictory.impossible);
}
