use super::*;

#[test]
fn a_unit_starts_unattributed() {
    let unit = WorkUnit::new("j1", "magistr", "ship it");
    assert_eq!(unit.id, "j1");
    assert_eq!(unit.project, "magistr");
    assert_eq!(unit.prompt, "ship it");
    assert!(unit.requested_by.is_empty());
}

#[test]
fn the_requester_is_a_builder_step() {
    let unit = WorkUnit::new("j1", "magistr", "ship it").requested_by("client-7");
    assert_eq!(unit.requested_by, "client-7");
}

#[test]
fn a_unit_from_a_wire_without_a_requester_still_parses() {
    let json = r#"{"id":"j1","project":"magistr","prompt":"ship it"}"#;
    let unit: WorkUnit = serde_json::from_str(json).expect("parses");
    assert_eq!(unit, WorkUnit::new("j1", "magistr", "ship it"));
}
