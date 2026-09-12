use super::*;

#[test]
fn a_chunk_names_itself_after_its_id_and_carries_its_record_shape() {
    let chunk = ProposedChunk::new("harmonic-engine", "core", 100).standing_on(&["import"]);
    assert_eq!(chunk.name, "harmonic engine");
    assert_eq!(chunk.row(), "core \u{b7} harmonic engine");
    let record = chunk.record();
    assert_eq!(record.id, "harmonic-engine");
    assert_eq!(
        record.title, "harmonic engine",
        "the name stands in for a missing claim"
    );
    assert_eq!(record.status, magistr_engine::ChunkStatus::Pending);
    assert_eq!(record.deps, vec!["import".to_owned()]);
    assert_eq!(record.group.as_deref(), Some("core"));
    assert!(!chunk.pinned);
}
