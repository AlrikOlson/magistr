use super::*;
use crate::roadmap::{Phase, PhaseStatus, Roadmap, Task};

fn task(text: &str, checked: bool) -> Task {
    Task {
        text: text.into(),
        checked,
        detail: String::new(),
        subtasks: vec![],
    }
}

fn task_with_subs(text: &str, checked: bool, subs: Vec<Task>) -> Task {
    Task {
        text: text.into(),
        checked,
        detail: String::new(),
        subtasks: subs,
    }
}

fn phase(id: &str, name: &str, tasks: Vec<Task>) -> Phase {
    Phase {
        id: id.into(),
        name: name.into(),
        tagline: None,
        status: PhaseStatus::Pending,
        problem: String::new(),
        solution: String::new(),
        depends_on: vec![],
        tasks,
    }
}

fn roadmap(phases: Vec<Phase>) -> Roadmap {
    Roadmap {
        title: "Test".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases,
        epilogue: String::new(),
    }
}

fn index(phases: Vec<Phase>) -> RoadmapIndex {
    RoadmapIndex::new(roadmap(phases))
}

// ── Stats ───────────────────────────────────────────────────────────────

#[test]
fn stats_empty() {
    let idx = index(vec![]);
    let s = idx.stats();
    assert_eq!(s.total_phases, 0);
    assert_eq!(s.total_tasks, 0);
    assert_eq!(s.progress(), 0.0);
}

#[test]
fn stats_with_data() {
    let idx = index(vec![
        phase("E1", "One", vec![task("A", true), task("B", false)]),
        phase("E2", "Two", vec![task("C", true)]),
    ]);
    let s = idx.stats();
    assert_eq!(s.total_phases, 2);
    assert_eq!(s.total_tasks, 3);
    assert_eq!(s.checked_tasks, 2);
}

#[test]
fn stats_completed_phases() {
    let mut p1 = phase("E1", "One", vec![task("A", true)]);
    p1.status = PhaseStatus::Done;
    let idx = index(vec![p1, phase("E2", "Two", vec![task("B", false)])]);
    assert_eq!(idx.stats().completed_phases, 1);
}

// ── Phase queries ───────────────────────────────────────────────────────

#[test]
fn phase_by_id_found() {
    let idx = index(vec![phase("E1", "One", vec![]), phase("E2", "Two", vec![])]);
    let (i, p) = idx.phase_by_id("E2").unwrap();
    assert_eq!(i, 1);
    assert_eq!(p.name, "Two");
}

#[test]
fn phase_by_id_not_found() {
    let idx = index(vec![phase("E1", "One", vec![])]);
    assert!(idx.phase_by_id("E99").is_none());
}

#[test]
fn tasks_for_phase() {
    let idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    let tasks = idx.tasks(0).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].text, "A");
}

#[test]
fn tasks_out_of_bounds() {
    let idx = index(vec![]);
    assert!(idx.tasks(0).is_none());
}

// ── Next unchecked ──────────────────────────────────────────────────────

#[test]
fn next_unchecked_skips_checked() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task("Done", true), task("Not done", false)],
    )]);
    let results = idx.next_unchecked(10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].text, "Not done");
}

#[test]
fn next_unchecked_respects_limit() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task("A", false), task("B", false), task("C", false)],
    )]);
    let results = idx.next_unchecked(2);
    assert_eq!(results.len(), 2);
}

#[test]
fn next_unchecked_includes_subtasks() {
    // Parent checked=true with unchecked subtask → sanitized to parent unchecked
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs("Parent", true, vec![task("Sub", false)])],
    )]);
    let results = idx.next_unchecked(10);
    // Both parent (sanitized to unchecked) and subtask appear
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].text, "Parent");
    assert_eq!(results[0].task_path, vec![0]);
    assert_eq!(results[1].text, "Sub");
    assert_eq!(results[1].task_path, vec![0, 0]);
}

// ── Find task ───────────────────────────────────────────────────────────

#[test]
fn find_task_case_insensitive() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![
            task("Define core Types", false),
            task("Something else", false),
        ],
    )]);
    let results = idx.find_task("CORE TYPES");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].text, "Define core Types");
}

#[test]
fn find_task_in_subtasks() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            false,
            vec![task("Hidden gem", false)],
        )],
    )]);
    let results = idx.find_task("gem");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].task_path, vec![0, 0]);
}

#[test]
fn find_task_no_match() {
    let idx = index(vec![phase("E1", "One", vec![task("Hello", false)])]);
    assert!(idx.find_task("nonexistent").is_empty());
}

// ── Toggle task ─────────────────────────────────────────────────────────

#[test]
fn toggle_task_top_level() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    let new_state = idx.toggle_task(0, &[0]).unwrap();
    assert!(new_state);
    assert!(idx.tasks(0).unwrap()[0].checked);
    assert!(idx.is_dirty());
}

#[test]
fn toggle_task_subtask() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs("Parent", false, vec![task("Sub", false)])],
    )]);
    let new_state = idx.toggle_task(0, &[0, 0]).unwrap();
    assert!(new_state);
    assert!(idx.tasks(0).unwrap()[0].subtasks[0].checked);
}

#[test]
fn toggle_task_recomputes_phase_status() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    idx.toggle_task(0, &[0]).unwrap();
    assert_eq!(idx.phases()[0].status, PhaseStatus::Done);
}

#[test]
fn toggle_task_invalid_phase() {
    let mut idx = index(vec![]);
    assert!(idx.toggle_task(0, &[0]).is_err());
}

#[test]
fn toggle_task_invalid_index() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    assert!(idx.toggle_task(0, &[5]).is_err());
}

#[test]
fn toggle_task_empty_path() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    assert!(idx.toggle_task(0, &[]).is_err());
}

// ── Update task text ────────────────────────────────────────────────────

#[test]
fn update_task_text_works() {
    let mut idx = index(vec![phase("E1", "One", vec![task("Old", false)])]);
    idx.update_task_text(0, &[0], "New".into()).unwrap();
    assert_eq!(idx.tasks(0).unwrap()[0].text, "New");
    assert!(idx.is_dirty());
}

// ── Update task detail ──────────────────────────────────────────────────

#[test]
fn update_task_detail_works() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    idx.update_task_detail(0, &[0], "Some detail".into())
        .unwrap();
    assert_eq!(idx.tasks(0).unwrap()[0].detail, "Some detail");
}

// ── Add task ────────────────────────────────────────────────────────────

#[test]
fn add_task_appends() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    idx.add_task(0, "B".into()).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[1].text, "B");
    assert!(!tasks[1].checked);
}

#[test]
fn add_task_invalid_phase() {
    let mut idx = index(vec![]);
    assert!(idx.add_task(0, "X".into()).is_err());
}

// ── Add subtask ─────────────────────────────────────────────────────────

#[test]
fn add_subtask_works() {
    let mut idx = index(vec![phase("E1", "One", vec![task("Parent", false)])]);
    idx.add_subtask(0, &[0], "Child".into()).unwrap();
    assert_eq!(idx.tasks(0).unwrap()[0].subtasks.len(), 1);
    assert_eq!(idx.tasks(0).unwrap()[0].subtasks[0].text, "Child");
}

// ── Delete task ─────────────────────────────────────────────────────────

#[test]
fn delete_task_top_level() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task("A", false), task("B", false)],
    )]);
    idx.delete_task(0, &[0]).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].text, "B");
}

#[test]
fn delete_task_subtask() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            false,
            vec![task("Sub1", false), task("Sub2", false)],
        )],
    )]);
    idx.delete_task(0, &[0, 0]).unwrap();
    assert_eq!(idx.tasks(0).unwrap()[0].subtasks.len(), 1);
    assert_eq!(idx.tasks(0).unwrap()[0].subtasks[0].text, "Sub2");
}

#[test]
fn delete_task_empty_path() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    assert!(idx.delete_task(0, &[]).is_err());
}

#[test]
fn delete_task_out_of_bounds() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    assert!(idx.delete_task(0, &[5]).is_err());
}

// ── Add phase ───────────────────────────────────────────────────────────

#[test]
fn add_phase_works() {
    let mut idx = index(vec![]);
    idx.add_phase(phase("E1", "New Phase", vec![])).unwrap();
    assert_eq!(idx.phases().len(), 1);
    assert_eq!(idx.phases()[0].id, "E1");
    assert!(idx.is_dirty());
}

#[test]
fn add_phase_duplicate_id() {
    let mut idx = index(vec![phase("E1", "Existing", vec![])]);
    assert!(idx.add_phase(phase("E1", "Dup", vec![])).is_err());
}

// ── Remove phase ────────────────────────────────────────────────────────

#[test]
fn remove_phase_works() {
    let mut idx = index(vec![phase("E1", "One", vec![]), phase("E2", "Two", vec![])]);
    idx.remove_phase(0).unwrap();
    assert_eq!(idx.phases().len(), 1);
    assert_eq!(idx.phases()[0].id, "E2");
}

#[test]
fn remove_phase_out_of_bounds() {
    let mut idx = index(vec![]);
    assert!(idx.remove_phase(0).is_err());
}

// ── Dirty flag ──────────────────────────────────────────────────────────

#[test]
fn starts_clean() {
    let idx = index(vec![]);
    assert!(!idx.is_dirty());
}

#[test]
fn mark_clean_resets() {
    let mut idx = index(vec![phase("E1", "One", vec![task("A", false)])]);
    idx.toggle_task(0, &[0]).unwrap();
    assert!(idx.is_dirty());
    idx.mark_clean();
    assert!(!idx.is_dirty());
}

// ── Task ID ─────────────────────────────────────────────────────────────

#[test]
fn task_id_top_level() {
    assert_eq!(task_id("E0", &[0]), "E0.0");
    assert_eq!(task_id("V1", &[2]), "V1.2");
}

#[test]
fn task_id_subtask() {
    assert_eq!(task_id("E0", &[0, 1]), "E0.0.1");
    assert_eq!(task_id("E0", &[2, 0, 3]), "E0.2.0.3");
}

#[test]
fn resolve_task_id_roundtrip() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            false,
            vec![task("Sub0", false), task("Sub1", false)],
        )],
    )]);

    let (pi, path) = idx.resolve_task_id("E1.0.1").unwrap();
    assert_eq!(pi, 0);
    assert_eq!(path, vec![0, 1]);
    assert_eq!(task_id("E1", &path), "E1.0.1");
}

#[test]
fn resolve_task_id_top_level() {
    let idx = index(vec![phase("V1", "First", vec![task("A", false)])]);
    let (pi, path) = idx.resolve_task_id("V1.0").unwrap();
    assert_eq!(pi, 0);
    assert_eq!(path, vec![0]);
}

#[test]
fn resolve_task_id_unknown_phase() {
    let idx = index(vec![phase("E1", "One", vec![])]);
    assert!(idx.resolve_task_id("E99.0").is_err());
}

#[test]
fn resolve_task_id_no_task_index() {
    let idx = index(vec![phase("E1", "One", vec![])]);
    assert!(idx.resolve_task_id("E1").is_err());
}

#[test]
fn resolve_task_id_invalid_index() {
    let idx = index(vec![phase("E1", "One", vec![])]);
    assert!(idx.resolve_task_id("E1.abc").is_err());
}

#[test]
fn next_unchecked_has_task_id() {
    // Use a consistent state (parent unchecked, subtask unchecked)
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs("Parent", false, vec![task("Sub", false)])],
    )]);
    let results = idx.next_unchecked(10);
    assert_eq!(results[0].task_id, "E1.0");
    assert_eq!(results[1].task_id, "E1.0.0");
}

#[test]
fn find_task_has_task_id() {
    let idx = index(vec![phase("V1", "First", vec![task("Hello world", false)])]);
    let results = idx.find_task("hello");
    assert_eq!(results[0].task_id, "V1.0");
}

// ── Cascading DOWN ──────────────────────────────────────────────────

#[test]
fn cascade_down_check_parent_checks_all_subtasks() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            false,
            vec![
                task("Sub1", false),
                task("Sub2", false),
                task("Sub3", false),
            ],
        )],
    )]);
    let new_state = idx.toggle_task(0, &[0]).unwrap();
    assert!(new_state);
    let tasks = idx.tasks(0).unwrap();
    assert!(tasks[0].checked);
    assert!(tasks[0].subtasks.iter().all(|s| s.checked));
    let stats = idx.stats();
    assert_eq!(stats.total_tasks, 4);
    assert_eq!(stats.checked_tasks, 4);
}

#[test]
fn cascade_down_uncheck_parent_unchecks_all_subtasks() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            true,
            vec![task("Sub1", true), task("Sub2", true)],
        )],
    )]);
    let new_state = idx.toggle_task(0, &[0]).unwrap();
    assert!(!new_state);
    let tasks = idx.tasks(0).unwrap();
    assert!(!tasks[0].checked);
    assert!(tasks[0].subtasks.iter().all(|s| !s.checked));
}

#[test]
fn cascade_down_nested_subtasks() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Root",
            false,
            vec![task_with_subs("Mid", false, vec![task("Leaf", false)])],
        )],
    )]);
    idx.toggle_task(0, &[0]).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert!(tasks[0].checked);
    assert!(tasks[0].subtasks[0].checked);
    assert!(tasks[0].subtasks[0].subtasks[0].checked);
}

// ── Cascading UP ────────────────────────────────────────────────────

#[test]
fn cascade_up_last_sibling_checks_parent() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            false,
            vec![task("Sub1", true), task("Sub2", false)],
        )],
    )]);
    // Check the last unchecked sibling
    idx.toggle_task(0, &[0, 1]).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert!(
        tasks[0].checked,
        "parent should auto-check when all children checked"
    );
}

#[test]
fn cascade_up_uncheck_subtask_unchecks_parent() {
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            true,
            vec![task("Sub1", true), task("Sub2", true)],
        )],
    )]);
    // Uncheck one subtask
    idx.toggle_task(0, &[0, 0]).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert!(
        !tasks[0].checked,
        "parent should auto-uncheck when a child is unchecked"
    );
}

#[test]
fn cascade_up_deep_nesting() {
    // Root -> Mid -> Leaf (all unchecked). Check Leaf → Mid and Root auto-check.
    let mut idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Root",
            false,
            vec![task_with_subs("Mid", false, vec![task("Leaf", false)])],
        )],
    )]);
    idx.toggle_task(0, &[0, 0, 0]).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert!(tasks[0].subtasks[0].subtasks[0].checked, "leaf checked");
    assert!(tasks[0].subtasks[0].checked, "mid auto-checked");
    assert!(tasks[0].checked, "root auto-checked");
}

// ── Sanitize on load ────────────────────────────────────────────────

#[test]
fn sanitize_fixes_parent_checked_children_unchecked() {
    // Contradictory state: parent checked but children unchecked
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            true,
            vec![task("Sub1", false), task("Sub2", true)],
        )],
    )]);
    // After sanitize, parent should be unchecked (not all children checked)
    let tasks = idx.tasks(0).unwrap();
    assert!(!tasks[0].checked, "parent sanitized to unchecked");
}

#[test]
fn sanitize_preserves_correct_state() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Parent",
            true,
            vec![task("Sub1", true), task("Sub2", true)],
        )],
    )]);
    let tasks = idx.tasks(0).unwrap();
    assert!(
        tasks[0].checked,
        "parent stays checked when all children checked"
    );
}

#[test]
fn sanitize_deep_nesting() {
    // Root(checked) -> Mid(checked) -> Leaf(unchecked)
    // Should fix Mid to unchecked, then Root to unchecked
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs(
            "Root",
            true,
            vec![task_with_subs("Mid", true, vec![task("Leaf", false)])],
        )],
    )]);
    let tasks = idx.tasks(0).unwrap();
    assert!(!tasks[0].subtasks[0].checked, "mid sanitized to unchecked");
    assert!(!tasks[0].checked, "root sanitized to unchecked");
}

#[test]
fn sanitize_leaf_unchanged() {
    // Leaf tasks without subtasks are never changed by sanitize
    let idx = index(vec![phase("E1", "One", vec![task("Leaf", true)])]);
    assert!(idx.tasks(0).unwrap()[0].checked);
}

// ── to_spec_summary ─────────────────────────────────────────────────

#[test]
fn to_spec_summary_matches_stats() {
    let idx = index(vec![
        phase(
            "E1",
            "One",
            vec![task("A", true), task("B", false), task("C", true)],
        ),
        phase("E2", "Two", vec![task("D", false)]),
    ]);
    let summary = idx.to_spec_summary();
    let stats = idx.stats();
    assert_eq!(summary.total, stats.total_tasks);
    assert_eq!(summary.checked, stats.checked_tasks);
    assert_eq!(summary.unchecked, stats.total_tasks - stats.checked_tasks);
}

#[test]
fn to_spec_summary_unchecked_items() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![
            task("Done", true),
            task("Todo1", false),
            task("Todo2", false),
        ],
    )]);
    let summary = idx.to_spec_summary();
    assert_eq!(summary.unchecked_items, vec!["Todo1", "Todo2"]);
}

#[test]
fn to_spec_summary_includes_subtasks() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task_with_subs("Parent", false, vec![task("Sub", false)])],
    )]);
    let summary = idx.to_spec_summary();
    assert_eq!(summary.total, 2);
    assert_eq!(summary.unchecked, 2);
    assert_eq!(summary.unchecked_items, vec!["Parent", "Sub"]);
}

#[test]
fn to_spec_summary_all_done() {
    let idx = index(vec![phase(
        "E1",
        "One",
        vec![task("A", true), task("B", true)],
    )]);
    let summary = idx.to_spec_summary();
    assert!(summary.all_done());
}

#[test]
fn to_spec_summary_empty() {
    let idx = index(vec![]);
    let summary = idx.to_spec_summary();
    assert_eq!(summary.total, 0);
    assert!(!summary.all_done());
}

// ── swap_tasks tests ────────────────────────────────────────────

#[test]
fn swap_tasks_within_phase() {
    let mut idx = index(vec![phase(
        "P1",
        "Phase",
        vec![task("A", false), task("B", false), task("C", false)],
    )]);

    idx.swap_tasks(0, 0, 2).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert_eq!(tasks[0].text, "C");
    assert_eq!(tasks[2].text, "A");
    assert!(idx.is_dirty());
}

#[test]
fn swap_tasks_adjacent() {
    let mut idx = index(vec![phase(
        "P1",
        "Phase",
        vec![task("A", false), task("B", false)],
    )]);

    idx.swap_tasks(0, 0, 1).unwrap();
    let tasks = idx.tasks(0).unwrap();
    assert_eq!(tasks[0].text, "B");
    assert_eq!(tasks[1].text, "A");
}

#[test]
fn swap_tasks_out_of_bounds() {
    let mut idx = index(vec![phase("P1", "Phase", vec![task("A", false)])]);
    assert!(idx.swap_tasks(0, 0, 5).is_err());
}

#[test]
fn swap_tasks_invalid_phase() {
    let mut idx = index(vec![]);
    assert!(idx.swap_tasks(0, 0, 1).is_err());
}

// ── move_task_between_phases tests ──────────────────────────────

#[test]
fn move_task_between_phases() {
    let mut idx = index(vec![
        phase("P1", "From", vec![task("A", false), task("B", false)]),
        phase("P2", "To", vec![task("C", false)]),
    ]);

    idx.move_task_between_phases(0, 0, 1).unwrap();
    assert_eq!(idx.tasks(0).unwrap().len(), 1);
    assert_eq!(idx.tasks(0).unwrap()[0].text, "B");
    assert_eq!(idx.tasks(1).unwrap().len(), 2);
    assert_eq!(idx.tasks(1).unwrap()[1].text, "A");
    assert!(idx.is_dirty());
}

#[test]
fn move_task_same_phase_is_noop() {
    let mut idx = index(vec![phase("P1", "Same", vec![task("A", false)])]);
    idx.move_task_between_phases(0, 0, 0).unwrap();
    assert!(!idx.is_dirty());
}

#[test]
fn move_task_invalid_phase() {
    let mut idx = index(vec![phase("P1", "Only", vec![task("A", false)])]);
    assert!(idx.move_task_between_phases(0, 0, 5).is_err());
}

#[test]
fn move_task_invalid_task_index() {
    let mut idx = index(vec![
        phase("P1", "From", vec![task("A", false)]),
        phase("P2", "To", vec![]),
    ]);
    assert!(idx.move_task_between_phases(0, 5, 1).is_err());
}

// ── Insert phase at position ────────────────────────────────────

#[test]
fn add_phase_before() {
    let mut idx = index(vec![
        phase("A", "Alpha", vec![]),
        phase("C", "Charlie", vec![]),
    ]);
    idx.add_phase_at(
        phase("B", "Bravo", vec![]),
        Some(InsertPosition::Before("C")),
    )
    .unwrap();
    let ids: Vec<&str> = idx.phases().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["A", "B", "C"]);
}

#[test]
fn add_phase_after() {
    let mut idx = index(vec![
        phase("A", "Alpha", vec![]),
        phase("C", "Charlie", vec![]),
    ]);
    idx.add_phase_at(
        phase("B", "Bravo", vec![]),
        Some(InsertPosition::After("A")),
    )
    .unwrap();
    let ids: Vec<&str> = idx.phases().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["A", "B", "C"]);
}

#[test]
fn add_phase_before_first() {
    let mut idx = index(vec![phase("B", "Bravo", vec![])]);
    idx.add_phase_at(
        phase("A", "Alpha", vec![]),
        Some(InsertPosition::Before("B")),
    )
    .unwrap();
    assert_eq!(idx.phases()[0].id, "A");
}

#[test]
fn add_phase_after_last() {
    let mut idx = index(vec![phase("A", "Alpha", vec![])]);
    idx.add_phase_at(
        phase("B", "Bravo", vec![]),
        Some(InsertPosition::After("A")),
    )
    .unwrap();
    assert_eq!(idx.phases()[1].id, "B");
}

#[test]
fn add_phase_at_unknown_ref() {
    let mut idx = index(vec![phase("A", "Alpha", vec![])]);
    assert!(
        idx.add_phase_at(
            phase("B", "Bravo", vec![]),
            Some(InsertPosition::Before("Z"))
        )
        .is_err()
    );
}

// ── Move phase ──────────────────────────────────────────────────

#[test]
fn move_phase_before() {
    let mut idx = index(vec![
        phase("A", "Alpha", vec![]),
        phase("B", "Bravo", vec![]),
        phase("C", "Charlie", vec![]),
    ]);
    idx.move_phase("C", InsertPosition::Before("A")).unwrap();
    let ids: Vec<&str> = idx.phases().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["C", "A", "B"]);
}

#[test]
fn move_phase_after() {
    let mut idx = index(vec![
        phase("A", "Alpha", vec![]),
        phase("B", "Bravo", vec![]),
        phase("C", "Charlie", vec![]),
    ]);
    idx.move_phase("A", InsertPosition::After("B")).unwrap();
    let ids: Vec<&str> = idx.phases().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["B", "A", "C"]);
}

#[test]
fn move_phase_to_end() {
    let mut idx = index(vec![
        phase("A", "Alpha", vec![]),
        phase("B", "Bravo", vec![]),
        phase("C", "Charlie", vec![]),
    ]);
    idx.move_phase("A", InsertPosition::After("C")).unwrap();
    let ids: Vec<&str> = idx.phases().iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["B", "C", "A"]);
}

#[test]
fn move_phase_unknown_source() {
    let mut idx = index(vec![phase("A", "Alpha", vec![])]);
    assert!(idx.move_phase("Z", InsertPosition::Before("A")).is_err());
}

#[test]
fn move_phase_unknown_target() {
    let mut idx = index(vec![phase("A", "Alpha", vec![])]);
    assert!(idx.move_phase("A", InsertPosition::Before("Z")).is_err());
}

// ── Update phase ────────────────────────────────────────────────

#[test]
fn update_phase_name() {
    let mut idx = index(vec![phase("E1", "Old Name", vec![])]);
    idx.update_phase("E1", Some("New Name"), None, None, None, None)
        .unwrap();
    assert_eq!(idx.phases()[0].name, "New Name");
    assert!(idx.is_dirty());
}

#[test]
fn update_phase_tagline() {
    let mut idx = index(vec![phase("E1", "Name", vec![])]);
    idx.update_phase("E1", None, Some(Some("A tagline")), None, None, None)
        .unwrap();
    assert_eq!(idx.phases()[0].tagline.as_deref(), Some("A tagline"));
}

#[test]
fn update_phase_clear_tagline() {
    let mut p = phase("E1", "Name", vec![]);
    p.tagline = Some("Old".into());
    let mut idx = index(vec![p]);
    idx.update_phase("E1", None, Some(None), None, None, None)
        .unwrap();
    assert!(idx.phases()[0].tagline.is_none());
}

#[test]
fn update_phase_problem_solution() {
    let mut idx = index(vec![phase("E1", "Name", vec![])]);
    idx.update_phase(
        "E1",
        None,
        None,
        Some("The problem"),
        Some("The solution"),
        None,
    )
    .unwrap();
    assert_eq!(idx.phases()[0].problem, "The problem");
    assert_eq!(idx.phases()[0].solution, "The solution");
}

#[test]
fn update_phase_unknown_id() {
    let mut idx = index(vec![phase("E1", "Name", vec![])]);
    assert!(
        idx.update_phase("Z", Some("X"), None, None, None, None)
            .is_err()
    );
}

// ── Remove phase by ID ──────────────────────────────────────────

#[test]
fn remove_phase_by_id_works() {
    let mut idx = index(vec![phase("E1", "One", vec![]), phase("E2", "Two", vec![])]);
    let removed = idx.remove_phase_by_id("E1").unwrap();
    assert_eq!(removed.id, "E1");
    assert_eq!(idx.phases().len(), 1);
    assert_eq!(idx.phases()[0].id, "E2");
}

#[test]
fn remove_phase_by_id_unknown() {
    let mut idx = index(vec![phase("E1", "One", vec![])]);
    assert!(idx.remove_phase_by_id("Z").is_err());
}
