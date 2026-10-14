use super::*;
use magistr_workflow::roadmap::RoadmapEngine;

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
fn snapshot_reports_counts_next_workstreams_and_progress() {
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

    let s = NativeRoadmapSnapshot::from_chunks(&e.roadmap().project_id, &e.roadmap().chunks);
    assert_eq!(s.project_id, "magistr-test");
    assert_eq!(s.count("pending"), 2);
    assert_eq!(s.count("in_progress"), 1);
    assert_eq!(s.count("obsoleted"), 1);
    assert_eq!(s.count("done"), 0);
    assert_eq!(s.in_progress, vec!["base".to_string()]);
    // gui is blocked on base (in progress); cli is the lowest ready.
    assert_eq!(s.next_ready.as_deref(), Some("cli"));
    let engine = s
        .workstreams
        .current
        .get("Engine")
        .expect("Engine workstream");
    assert_eq!(engine.counts.total, 2);
    assert_eq!(engine.counts.in_progress, 1);
    assert_eq!(engine.counts.pending, 1);
    assert_eq!(engine.readiness.ready, 1);
    assert_eq!(engine.next_ready.as_deref(), Some("cli"));
    let desktop = &s.workstreams.current["Desktop"];
    assert_eq!(desktop.counts.pending, 1);
    assert_eq!(desktop.readiness.waiting, 1);
    assert_eq!(desktop.readiness.blocked, 0);
    assert_eq!(s.workstreams.ungrouped_history.obsoleted, 1);
    assert_eq!(s.progress.total_tasks, 3);
    assert_eq!(s.progress.total_phases, 2);
}

#[test]
fn snapshot_round_trips_through_json() {
    let mut e = RoadmapEngine::new("p".into());
    add(&mut e, "a", 1, &[]);
    let s = NativeRoadmapSnapshot::from_chunks(&e.roadmap().project_id, &e.roadmap().chunks);
    let json = serde_json::to_string(&s).expect("ser");
    let back: NativeRoadmapSnapshot = serde_json::from_str(&json).expect("de");
    assert_eq!(back, s);
}
