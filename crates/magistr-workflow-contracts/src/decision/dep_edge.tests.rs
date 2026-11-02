use super::*;

fn parse(json: &str) -> DepEdge {
    serde_json::from_str(json).expect("DepEdge should deserialize")
}

#[test]
fn bare_integer() {
    assert_eq!(parse("5").note(), 5);
    assert_eq!(parse("5").relation(), None);
}

#[test]
fn tagged_note_field() {
    let e = parse(r#"{"note":7,"relation":"supports"}"#);
    assert_eq!(e.note(), 7);
    assert_eq!(e.relation(), Some("supports"));
}

#[test]
fn on_and_from_aliases_for_note() {
    // Agents naturally write {on: N} / {from: N}; both alias to `note`.
    assert_eq!(parse(r#"{"on":48}"#).note(), 48);
    assert_eq!(parse(r#"{"from":12}"#).note(), 12);
    assert_eq!(
        parse(r#"{"on":48,"relation":"depends_on"}"#).relation(),
        Some("depends_on")
    );
}
