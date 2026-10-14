use super::*;
use magistr_engine::native::{NativeConfig, NativeEngines, ProjectAuthority, WriterRole};

fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    std::fs::create_dir(tmp.path().join("proj")).expect("project directory");
    let cfg = NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
        .with_data_dir(tmp.path().join("data"));
    ProjectAuthority::admit_for_project(&cfg, "render-test".into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn add(e: &NativeEngines, id: &str, priority: u32, deps: &[&str], group: Option<&str>) {
    e.roadmap_add_chunk(
        id.into(),
        format!("Chunk {id}"),
        String::new(),
        ChunkStatus::Pending,
        priority,
        format!("About {id}."),
        vec![format!("{id} works")],
        deps.iter().map(|d| (*d).to_string()).collect(),
        None,
        group.map(str::to_owned),
    )
    .expect("add chunk");
}

#[test]
fn show_lists_counts_workstreams_next_and_pending_in_priority_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "base", 100, &[], Some("Core"));
    add(&e, "later", 300, &["base"], Some("Core"));
    add(&e, "side", 200, &[], None);
    e.roadmap_start_chunk("side").expect("start");

    let text = render_show(&e.roadmap_snapshot().unwrap(), &e.roadmap_chunks().unwrap());
    assert!(text.contains("render-test"), "{text}");
    assert!(text.contains("0/3 chunks done (0%)"), "{text}");
    assert!(text.contains("pending 2 · in progress 1"), "{text}");
    assert!(
        text.contains("Core: 2 unfinished · 1 ready · 1 waiting · 0 blocked"),
        "{text}"
    );
    assert!(text.contains("Next ready: base — Chunk base"), "{text}");
    let base_at = text.find("[100] base").expect("base listed");
    let later_at = text.find("[300] later").expect("later listed");
    assert!(base_at < later_at, "priority order: {text}");
    assert!(
        !text.contains("[200] side"),
        "in-progress chunks are not pending: {text}"
    );
}

#[test]
fn show_keeps_closed_workstreams_in_history_and_ungrouped_work_in_global_next() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "old", 1, &[], Some("Closed"));
    add(&e, "grouped", 30, &[], Some("Current"));
    add(&e, "ungrouped", 10, &[], None);
    e.roadmap_set_status("old", ChunkStatus::Obsoleted).unwrap();
    let chunks = e.roadmap_chunks().unwrap();
    let snapshot = NativeRoadmapSnapshot::from_chunks(e.project_id(), &chunks);
    let text = render_show(&snapshot, &chunks);
    let (current, history) = text.split_once("History:").expect("explicit history");
    assert!(
        current.contains("Current: 1 unfinished · 1 ready · 0 waiting · 0 blocked"),
        "{text}"
    );
    assert!(!current.contains("Closed"), "{text}");
    assert!(
        current.contains("Ungrouped work: 1 unfinished · 1 ready"),
        "{text}"
    );
    assert!(history.contains("Closed: 0 done · 1 obsoleted"), "{text}");
    assert!(
        text.contains("Next ready: ungrouped — Chunk ungrouped"),
        "{text}"
    );

    let json = show_json(&snapshot, &chunks);
    assert_eq!(
        json["workstreams"]["current"]["Current"]["counts"]["total"],
        1
    );
    assert!(json["workstreams"]["current"].get("Closed").is_none());
    assert_eq!(json["workstreams"]["history"]["Closed"]["obsoleted"], 1);
    assert_eq!(json["workstreams"]["ungrouped"]["counts"]["total"], 1);
    assert_eq!(json["next_ready"], "ungrouped");
    assert!(json.get("regions").is_none());
    assert_eq!(chunks.len(), 3, "history remains in the owning record");
}

#[test]
fn show_reports_when_nothing_is_ready() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "base", 100, &[], None);
    add(&e, "later", 300, &["base"], None);
    e.roadmap_start_chunk("base").expect("start");
    let text = render_show(&e.roadmap_snapshot().unwrap(), &e.roadmap_chunks().unwrap());
    assert!(text.contains("Next ready: none"), "{text}");
}

#[test]
fn next_renders_the_chunk_or_explains_the_empty_frontier() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    assert!(render_next(None).contains("No chunk is ready"));
    add(&e, "base", 100, &["dep"], Some("Core"));
    add(&e, "dep", 50, &[], Some("Core"));
    let next = e.roadmap_next().unwrap().expect("dep is ready");
    let text = render_next(Some(&next));
    assert!(text.starts_with("dep — Chunk dep\n"), "{text}");
    assert!(text.contains("workstream: Core"), "{text}");
    assert!(text.contains("- dep works"), "{text}");
    let base = e.roadmap_chunk("base").unwrap().expect("base");
    assert!(render_next(Some(&base)).contains("deps: dep"));
}

#[test]
fn show_json_carries_the_snapshot_and_sorted_pending() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "b", 300, &[], None);
    add(&e, "a", 100, &[], None);
    let json = show_json(&e.roadmap_snapshot().unwrap(), &e.roadmap_chunks().unwrap());
    assert_eq!(json["backend"], "think-and-ship");
    assert_eq!(json["counts"]["pending"], 2);
    assert_eq!(json["next_ready"], "a");
    assert_eq!(json["pending"][0]["id"], "a");
    assert_eq!(json["pending"][1]["id"], "b");
    assert_eq!(json["pending"][0]["blocked"], false);
}

#[test]
fn native_required_names_the_flag() {
    assert!(NATIVE_REQUIRED.contains("roadmap_backend"));
    assert!(NATIVE_REQUIRED.contains("think-and-ship"));
}
