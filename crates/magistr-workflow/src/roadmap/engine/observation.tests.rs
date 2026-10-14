use super::*;
use crate::infra::{Domain, Persistence, PersistenceConfig};
use crate::roadmap::domain::ChunkStatus;

fn setup() -> (tempfile::TempDir, RoadmapEngine, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(
        &PersistenceConfig {
            enabled: true,
            data_dir: dir.path().to_owned(),
        },
        Domain::Roadmap,
    );
    let path = persistence.path_for("project");
    let engine = RoadmapEngine::new("project".into()).with_persistence(persistence);
    (dir, engine, path)
}

fn add(engine: &mut RoadmapEngine, id: &str) {
    engine
        .add_chunk(
            id.into(),
            "Saved work".into(),
            ChunkStatus::Pending,
            1,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .unwrap();
}

#[test]
fn observation_reads_durable_data_without_erasing_unsaved_writer_memory() {
    let (_dir, mut engine, path) = setup();
    add(&mut engine, "saved");
    let lock = path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    add(&mut engine, "unsaved");
    assert_eq!(engine.roadmap().chunks.len(), 2);
    let observed = engine.observe().unwrap();
    assert_eq!(observed.value.chunks.len(), 1);
    assert_eq!(observed.value.chunks[0].id, "saved");
    assert_eq!(
        engine.roadmap().chunks.len(),
        2,
        "a read preserves live writer state"
    );
}

#[test]
fn invalid_origins_are_refused_without_repairing_the_file() {
    let (_dir, mut engine, path) = setup();
    add(&mut engine, "saved");
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for value in [serde_json::Value::Null, serde_json::json!("elsewhere")] {
        let mut record = original.clone();
        record["chunks"][0]["project_id"] = value;
        let bytes = serde_json::to_vec(&record).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(engine.observe().unwrap_err().kind, ReadErrorKind::Identity);
        assert!(engine.refresh_if_stale().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn missing_and_successfully_empty_stores_have_different_sources() {
    let (_dir, engine, path) = setup();
    assert_eq!(engine.observe().unwrap().source, ReadSource::Missing);
    std::fs::write(&path, serde_json::to_vec(engine.roadmap()).unwrap()).unwrap();
    let read = engine.observe().unwrap();
    assert!(read.value.chunks.is_empty());
    assert!(matches!(read.source, ReadSource::File(_)));
}
