use super::*;
#[test]
fn references_and_kinds_round_trip_under_the_existing_wire_keys() {
    let wire = serde_json::json!({"type":"commit","ref":"abc123","description":"Changes"});
    let artifact: Artifact = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(artifact.reference, "abc123");
    assert_eq!(artifact.artifact_type, ArtifactType::Commit);
    assert_eq!(serde_json::to_value(artifact).unwrap(), wire);
}
