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
fn picker_lists_only_current_authored_names_and_global_selection() {
    use magistr_engine::native::{Chunk, ChunkStatus, workstream_catalog};

    let chunk = |id: &str, status: ChunkStatus, group: Option<&str>| -> Chunk {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "title": format!("Work {id}"),
            "status": status, "priority": 10, "group": group,
            "description": "", "notes": "", "acceptance": [], "deps": [],
            "cross_refs": [], "shared": false,
            "created_at": "2026-10-14T00:00:00Z", "updated_at": "2026-10-14T00:00:00Z"
        }))
        .unwrap()
    };
    let current = chunk("current", ChunkStatus::Pending, Some("Current"));
    let closed = chunk("closed", ChunkStatus::Obsoleted, Some("Closed"));
    let ungrouped = chunk("ungrouped", ChunkStatus::Pending, None);
    let catalog = workstream_catalog(&[current, closed, ungrouped]);
    let mut output = Vec::new();
    let selected = choose(
        &catalog.current_names(),
        &mut io::Cursor::new("0\n"),
        &mut output,
    )
    .unwrap();
    assert!(selected.is_none());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("1. Current"));
    assert!(output.contains("0. All tracks"));
    assert!(!output.contains("Closed"));
    assert!(!output.contains("Ungrouped"));
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
