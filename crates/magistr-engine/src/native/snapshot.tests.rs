use super::*;

fn add(e: &mut RoadmapEngine, id: &str, priority: u32, deps: &[&str]) {
    e.add_chunk(
        id.into(),
        format!("Chunk {id}"),
        ChunkStatus::Pending,
        priority,
        String::new(),
        vec![],
        deps.iter().map(|d| (*d).to_string()).collect(),
        false,
    )
    .expect("add");
}

#[test]
fn snapshot_reports_counts_next_regions_and_progress() {
    let mut e = RoadmapEngine::new("magistr-test".into());
    add(&mut e, "base", 100, &[]);
    add(&mut e, "gui", 200, &["base"]);
    add(&mut e, "cli", 300, &[]);
    add(&mut e, "old", 400, &[]);
    e.set_group("base", Some("Engine".into())).expect("g");
    e.set_group("gui", Some("Desktop".into())).expect("g");
    e.set_group("cli", Some("Engine".into())).expect("g");
    e.set_status("base", ChunkStatus::InProgress).expect("s");
    e.obsolete_chunk("old", "superseded".into()).expect("o");

    let s = NativeRoadmapSnapshot::from_engine(&e);
    assert_eq!(s.project_id, "magistr-test");
    assert_eq!(s.count("pending"), 2);
    assert_eq!(s.count("in_progress"), 1);
    assert_eq!(s.count("obsoleted"), 1);
    assert_eq!(s.count("done"), 0);
    assert_eq!(s.in_progress, vec!["base".to_string()]);
    // gui is blocked on base (in progress); cli is the lowest ready.
    assert_eq!(s.next_ready.as_deref(), Some("cli"));
    let engine = s.regions.get("Engine").expect("Engine region");
    assert_eq!(engine.total, 2);
    assert_eq!(engine.in_progress, 1);
    assert_eq!(engine.pending, 1);
    assert!(!engine.is_complete());
    assert_eq!(s.regions.get("Desktop").map(|r| r.pending), Some(1));
    assert!(!s.regions.contains_key("old"));
    assert_eq!(s.progress.total_tasks, 3);
    assert_eq!(s.progress.total_phases, 2);
}

#[test]
fn snapshot_round_trips_through_json() {
    let mut e = RoadmapEngine::new("p".into());
    add(&mut e, "a", 1, &[]);
    let s = NativeRoadmapSnapshot::from_engine(&e);
    let json = serde_json::to_string(&s).expect("ser");
    let back: NativeRoadmapSnapshot = serde_json::from_str(&json).expect("de");
    assert_eq!(back, s);
}
