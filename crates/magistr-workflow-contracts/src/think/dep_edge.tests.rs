use super::*;

fn parse(json: &str) -> DepEdge {
    serde_json::from_str(json).expect("DepEdge should deserialize")
}

#[test]
fn bare_integer() {
    assert_eq!(parse("5").step(), 5);
    assert_eq!(parse("5").relation(), None);
}

#[test]
fn tagged_step_field() {
    let e = parse(r#"{"step":7,"relation":"supports"}"#);
    assert_eq!(e.step(), 7);
    assert_eq!(e.relation(), Some("supports"));
}

#[test]
fn on_and_from_aliases_for_step() {
    // Agents naturally write {on: N} / {from: N}; both alias to `step`.
    assert_eq!(parse(r#"{"on":48}"#).step(), 48);
    assert_eq!(parse(r#"{"from":12}"#).step(), 12);
    assert_eq!(
        parse(r#"{"on":48,"relation":"depends_on"}"#).relation(),
        Some("depends_on")
    );
}
