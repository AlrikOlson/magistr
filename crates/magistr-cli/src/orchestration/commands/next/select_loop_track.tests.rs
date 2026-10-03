use super::*;

#[test]
fn choosing_a_track_reprompts_for_invalid_input_and_keeps_the_stored_name() {
    let tracks = vec!["Surface semantics".into(), "Native window".into()];
    let mut output = Vec::new();
    let chosen = choose(&tracks, &mut io::Cursor::new("wrong\n99\n2\n"), &mut output).unwrap();
    assert_eq!(chosen.as_deref(), Some("Native window"));
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("1. Surface semantics"));
    assert!(output.contains("Choose one of the listed numbers."));
}

#[test]
fn all_tracks_is_explicit_and_closed_input_starts_nothing() {
    let tracks = vec!["Surface semantics".into()];
    assert_eq!(
        choose(&tracks, &mut io::Cursor::new("0\n"), &mut Vec::new()).unwrap(),
        None
    );
    assert!(choose(&tracks, &mut io::Cursor::new(""), &mut Vec::new()).is_err());
}
