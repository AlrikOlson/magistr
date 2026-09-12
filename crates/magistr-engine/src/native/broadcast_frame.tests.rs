use super::*;

#[test]
fn parses_family_and_type() {
    let line = r#"{"family":"roadmap","type":"chunk_changed","chunk":{"id":"a"}}"#;
    let f = NativeFrame::parse_line(line).expect("frame");
    assert_eq!(f.family, "roadmap");
    assert_eq!(f.kind, "chunk_changed");
    assert!(f.is_roadmap());
    assert_eq!(f.payload["chunk"]["id"], "a");
}

#[test]
fn missing_type_is_an_empty_kind() {
    let f = NativeFrame::parse_line("{\"family\":\"ship\"}\n").expect("frame");
    assert_eq!(f.family, "ship");
    assert_eq!(f.kind, "");
    assert!(!f.is_roadmap());
}

#[test]
fn junk_lines_are_dropped() {
    assert!(NativeFrame::parse_line("").is_none());
    assert!(NativeFrame::parse_line("not json").is_none());
    assert!(NativeFrame::parse_line("[1,2]").is_none());
    assert!(NativeFrame::parse_line("{\"type\":\"x\"}").is_none());
    assert!(NativeFrame::parse_line("{\"family\":3}").is_none());
}
