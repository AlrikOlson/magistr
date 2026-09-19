const SOURCE: &str = include_str!("session_judge_for.rs");

#[test]
fn the_judge_holds_no_source_write_authority() {
    assert_eq!(
        SOURCE.matches("Connector::live(").count(),
        1,
        "the judge composes exactly one connector"
    );
    assert!(
        SOURCE.contains("Connector::live(SourceRoute::Refused(SourceRefusal::NoAuthority))"),
        "the judge's sessions are composed with the no-authority route, never the loop's writer"
    );
}
