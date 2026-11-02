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

fn add(e: &NativeEngines, id: &str, priority: u32, deps: &[&str], feature: Option<&str>) {
    e.plan_add_task(
        id.into(),
        format!("Task {id}"),
        String::new(),
        TaskStatus::ToDo,
        priority,
        format!("About {id}."),
        vec![format!("{id} works")],
        deps.iter().map(|d| (*d).to_string()).collect(),
        None,
        feature.map(str::to_owned),
    )
    .expect("add task");
}

#[test]
fn show_lists_counts_features_next_and_pending_in_priority_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "base", 100, &[], Some("Core"));
    add(&e, "later", 300, &["base"], Some("Core"));
    add(&e, "side", 200, &[], None);
    e.plan_start_task("side").expect("start");

    let text = render_show(&e.plan_snapshot().unwrap(), &e.plan_tasks().unwrap());
    assert!(text.contains("render-test"), "{text}");
    assert!(text.contains("0/3 tasks done (0%)"), "{text}");
    assert!(text.contains("to do 2 · in progress 1"), "{text}");
    assert!(
        text.contains("Core: 2 unfinished · 1 ready · 1 after other tasks · 0 waiting"),
        "{text}"
    );
    assert!(text.contains("Next ready: base — Task base"), "{text}");
    let base_at = text.find("[100] base").expect("base listed");
    let later_at = text.find("[300] later").expect("later listed");
    assert!(base_at < later_at, "priority order: {text}");
    assert!(
        !text.contains("[200] side"),
        "in-progress tasks are not pending: {text}"
    );
}

#[test]
fn show_keeps_closed_features_in_history_and_unassigned_work_in_global_next() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "old", 1, &[], Some("Closed"));
    add(&e, "grouped", 30, &[], Some("Current"));
    add(&e, "unassigned", 10, &[], None);
    e.plan_set_status("old", TaskStatus::Retired).unwrap();
    let tasks = e.plan_tasks().unwrap();
    let snapshot = NativePlanSnapshot::from_tasks(e.project_id(), &tasks);
    let text = render_show(&snapshot, &tasks);
    let (current, history) = text.split_once("History:").expect("explicit history");
    assert!(
        current.contains("Current: 1 unfinished · 1 ready · 0 after other tasks · 0 waiting"),
        "{text}"
    );
    assert!(!current.contains("Closed"), "{text}");
    assert!(
        current.contains("Unassigned work: 1 unfinished · 1 ready"),
        "{text}"
    );
    assert!(history.contains("Closed: 0 done · 1 retired"), "{text}");
    assert!(
        text.contains("Next ready: unassigned — Task unassigned"),
        "{text}"
    );

    let json = show_json(&snapshot, &tasks);
    assert_eq!(json["features"]["current"]["Current"]["counts"]["total"], 1);
    assert!(json["features"]["current"].get("Closed").is_none());
    assert_eq!(json["features"]["history"]["Closed"]["retired"], 1);
    assert_eq!(json["features"]["unassigned"]["counts"]["total"], 1);
    assert_eq!(json["next_ready"], "unassigned");
    assert!(json.get("regions").is_none());
    assert_eq!(tasks.len(), 3, "history remains in the owning record");
}

#[test]
fn show_reports_when_nothing_is_ready() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "base", 100, &[], None);
    add(&e, "later", 300, &["base"], None);
    e.plan_start_task("base").expect("start");
    let text = render_show(&e.plan_snapshot().unwrap(), &e.plan_tasks().unwrap());
    assert!(text.contains("Next ready: none"), "{text}");
}

#[test]
fn next_renders_the_task_or_explains_the_empty_frontier() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    assert!(render_next(None).contains("No task is ready"));
    add(&e, "base", 100, &["dep"], Some("Core"));
    add(&e, "dep", 50, &[], Some("Core"));
    let next = e.plan_next().unwrap().expect("dep is ready");
    let text = render_next(Some(&next));
    assert!(text.starts_with("dep — Task dep\n"), "{text}");
    assert!(text.contains("feature: Core"), "{text}");
    assert!(text.contains("- dep works"), "{text}");
    let base = e.plan_task("base").unwrap().expect("base");
    assert!(render_next(Some(&base)).contains("deps: dep"));
}

#[test]
fn show_json_carries_the_snapshot_and_sorted_pending() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let e = engines(&tmp);
    add(&e, "b", 300, &[], None);
    add(&e, "a", 100, &[], None);
    let json = show_json(&e.plan_snapshot().unwrap(), &e.plan_tasks().unwrap());
    assert_eq!(json["backend"], "magistr");
    assert_eq!(json["counts"]["to_do"], 2);
    assert_eq!(json["next_ready"], "a");
    assert_eq!(json["to_do"][0]["id"], "a");
    assert_eq!(json["to_do"][1]["id"], "b");
    assert_eq!(json["to_do"][0]["waiting"], false);
}

#[test]
fn native_required_names_the_flag() {
    assert!(NATIVE_REQUIRED.contains("plan_backend"));
    assert!(NATIVE_REQUIRED.contains("magistr"));
}
