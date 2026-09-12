use super::*;
use magistr_engine::native::ChunkStatus;
use magistr_engine::{NativeConfig, NativeEngines, ProjectAuthority, WriterRole};

fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    let cfg =
        NativeConfig::from_lookup(tmp.path(), |_| None).with_data_dir(tmp.path().join("tas-data"));
    ProjectAuthority::admit_for_project(&cfg, "native-summary".into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn add(engines: &NativeEngines, id: &str, priority: u32, deps: &[&str]) {
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|d| (*d).to_string()).collect(),
            None,
            None,
        )
        .expect("add chunk");
}

#[test]
fn counts_chunks_and_names_the_next_ready_one() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "first", 100, &[]);
    add(&e, "second", 200, &["first"]);

    let s = native_spec_summary(&e);
    assert_eq!((s.total, s.checked, s.unchecked), (2, 0, 1));
    assert_eq!(s.unchecked_items, vec!["first — Chunk first".to_string()]);
    assert!(!s.all_done());

    e.roadmap_start_chunk("first").unwrap();
    e.roadmap_complete_chunk("first", None).unwrap();
    let s = native_spec_summary(&e);
    assert_eq!((s.total, s.checked), (2, 1));
    assert_eq!(s.unchecked_items, vec!["second — Chunk second".to_string()]);
}

#[test]
fn nothing_ready_reads_as_all_done() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "only", 100, &[]);
    e.roadmap_start_chunk("only").unwrap();
    // In progress, not done: no chunk is ready, so the loop must stop.
    let s = native_spec_summary(&e);
    assert_eq!((s.total, s.checked, s.unchecked), (1, 0, 0));
    assert!(s.all_done());
}
