use super::*;

#[test]
fn a_chunk_names_itself_after_its_id_and_carries_its_wire_shape() {
    let chunk = ProposedChunk::new("harmonic-engine", "core", 100).standing_on(&["import"]);
    assert_eq!(chunk.name, "harmonic engine");
    assert_eq!(chunk.row(), "core \u{b7} harmonic engine");
    let wire = chunk.wire();
    assert_eq!(wire["id"], "harmonic-engine");
    assert_eq!(
        wire["title"], "harmonic engine",
        "the name stands in for a missing claim"
    );
    assert_eq!(wire["status"], "pending");
    assert_eq!(wire["deps"][0], "import");
    assert_eq!(wire["group"], "core");
    assert!(!chunk.pinned);
}
