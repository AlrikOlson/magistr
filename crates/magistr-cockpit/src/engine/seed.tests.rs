use magistr_engine::{ActionType, ChunkStatus, SignalKind, TasConfig, TaskType};

use super::*;

/// In-memory engines on a throwaway project directory.
fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let config = TasConfig::from_lookup(&root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .persist(false);
    NativeEngines::open(&config)
}

#[test]
fn an_empty_store_seeds_an_empty_rail() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let seed = StoreSeed::read(&engines(&tmp));
    assert_eq!(seed.chunk, None);
    assert_eq!((seed.done, seed.total), (0, 0));
    assert_eq!(seed.signals_pending, 0);
    assert_eq!(seed.ship_status["objective"], serde_json::Value::Null);
}

#[test]
fn reads_the_objective_the_chunk_and_the_pending_signals() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    engines.ship_set_objective(
        "ship the rail".to_owned(),
        vec!["it shows".to_owned()],
        Vec::new(),
        "chunk:r1".to_owned(),
    );
    engines.ship_add_task(
        "t1".to_owned(),
        "implement".to_owned(),
        TaskType::Implement,
        None,
    );
    engines
        .roadmap_add_chunk(
            "r0".to_owned(),
            "done already".to_owned(),
            String::new(),
            ChunkStatus::Done,
            1,
            String::new(),
            Vec::new(),
            Vec::new(),
            None,
            None,
        )
        .expect("chunk r0");
    engines
        .roadmap_add_chunk(
            "r1".to_owned(),
            "the rail".to_owned(),
            String::new(),
            ChunkStatus::Pending,
            2,
            String::new(),
            Vec::new(),
            Vec::new(),
            None,
            None,
        )
        .expect("chunk r1");
    engines.roadmap_start_chunk("r1").expect("start r1");
    engines.signal_capture(SignalKind::Idea, "user".to_owned(), "faster".to_owned());

    let seed = StoreSeed::read(&engines);
    assert_eq!(
        seed.ship_status["objective"]["description"],
        serde_json::json!("ship the rail")
    );
    assert_eq!(seed.chunk.as_deref(), Some("r1"));
    assert_eq!((seed.done, seed.total), (1, 2));
    assert_eq!(seed.signals_pending, 1);
}

/// Persisting engines on a throwaway store: `think_steps_from_store`
/// reads the archive, so the trace half only exists with persistence on.
fn persisted(tmp: &tempfile::TempDir) -> NativeEngines {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let config = TasConfig::from_lookup(&root, |_| None).with_data_dir(tmp.path().join("data"));
    NativeEngines::open_for_project(&config, "trace-seed".to_owned())
}

fn think_step(number: u32, purpose: &str) -> magistr_engine::ThinkStep {
    serde_json::from_value(serde_json::json!({
        "step_number": number,
        "estimated_total": 2,
        "purpose": purpose,
        "context": "the seed test",
        "thought": "Reasoning body.",
        "outcome": "Recorded.",
        "next_action": "Continue.",
        "rationale": "Because the test says so.",
    }))
    .expect("step")
}

#[test]
fn an_empty_store_seeds_an_empty_trace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert!(StoreSeed::read(&engines(&tmp)).trace.is_empty());
}

#[test]
fn the_stored_steps_come_back_as_step_appended_frames() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = persisted(&tmp);
    assert!(engines.think_record_step(think_step(1, "open")).is_ok());
    assert!(engines.think_record_step(think_step(2, "ground")).is_ok());

    let trace = StoreSeed::read(&engines).trace;
    assert_eq!(trace.len(), 2, "one frame per stored step");
    for frame in &trace {
        assert_eq!(
            (frame.family.as_str(), frame.kind.as_str()),
            ("think", "step_appended")
        );
        assert_eq!(frame.payload["family"], "think");
        assert_eq!(frame.payload["type"], "step_appended");
    }
    let numbers: Vec<u64> = trace
        .iter()
        .filter_map(|frame| frame.payload["step"]["step_number"].as_u64())
        .collect();
    assert_eq!(numbers, vec![1, 2], "in stored order");
    assert_eq!(trace[0].payload["step"]["purpose"], "open");
}

#[test]
fn the_ship_timeline_comes_back_as_action_and_check_frames() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = persisted(&tmp);
    engines.ship_set_objective(
        "seed the trace".to_owned(),
        Vec::new(),
        Vec::new(),
        "chunk:tui2-5a".to_owned(),
    );
    engines.ship_add_task(
        "t1".to_owned(),
        "implement".to_owned(),
        TaskType::Implement,
        None,
    );
    engines.ship_start_task("t1").expect("start t1");
    engines
        .ship_record_action(
            Some("t1"),
            ActionType::Code,
            "wrote the seed".to_owned(),
            vec!["seed.rs".to_owned()],
            None,
        )
        .expect("action");
    engines
        .ship_record_gate(
            Some("t1"),
            "cargo nextest",
            "cargo nextest run".to_owned(),
            0,
            "5125 passed".to_owned(),
            true,
            None,
        )
        .expect("gate");

    let trace = StoreSeed::read(&engines).trace;
    let kinds: Vec<&str> = trace.iter().map(|frame| frame.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec!["action_recorded", "check_recorded"],
        "the timeline in timestamp order"
    );
    assert_eq!(trace[0].payload["task_id"], "t1");
    assert_eq!(trace[0].payload["action"]["description"], "wrote the seed");
    assert_eq!(trace[1].payload["check"]["name"], "cargo nextest");
    // The exit code ship_status would have dropped.
    assert_eq!(trace[1].payload["check"]["exit_code"], 0);
}

#[test]
fn a_cockpit_opened_on_the_store_shows_its_trace_before_any_frame() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = persisted(&tmp);
    assert!(engines.think_record_step(think_step(1, "open")).is_ok());
    assert!(engines.think_record_step(think_step(2, "close")).is_ok());
    engines.ship_set_objective(
        "seed the trace".to_owned(),
        Vec::new(),
        Vec::new(),
        "chunk:tui2-5a".to_owned(),
    );
    engines.ship_add_task(
        "t1".to_owned(),
        "implement".to_owned(),
        TaskType::Implement,
        None,
    );
    engines
        .ship_record_gate(
            Some("t1"),
            "clippy",
            "cargo clippy".to_owned(),
            0,
            String::new(),
            true,
            None,
        )
        .expect("gate");

    // What every entry point does before the terminal is taken over —
    // no feed attached, no frame delivered.
    let model = crate::cockpit::Model::new(crate::cockpit::Size::new(120, 40))
        .with_store_seed(&StoreSeed::read(&engines));

    let trace = model.trace();
    assert!(!trace.is_empty(), "the Trace tab starts populated");
    assert_eq!(
        trace.steps().iter().map(|s| s.number).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(trace.step(2).expect("step 2").purpose, "close");
    assert_eq!(trace.moments().len(), 1, "the gate is on the timeline");
}
