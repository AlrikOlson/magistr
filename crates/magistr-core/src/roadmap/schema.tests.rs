use super::*;
use crate::roadmap::{Phase, PhaseStatus, Task};

fn sample_task(text: &str, checked: bool) -> Task {
    Task {
        text: text.into(),
        checked,
        detail: String::new(),
        subtasks: vec![],
    }
}

fn sample_phase(id: &str, name: &str, tasks: Vec<Task>) -> Phase {
    Phase {
        id: id.into(),
        name: name.into(),
        tagline: None,
        status: PhaseStatus::default(),
        problem: String::new(),
        solution: String::new(),
        depends_on: vec![],
        tasks,
    }
}

fn sample_roadmap(phases: Vec<Phase>) -> Roadmap {
    Roadmap {
        title: "Test Roadmap".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases,
        epilogue: String::new(),
    }
}

// ── Serde round-trip ────────────────────────────────────────────────────

#[test]
fn serde_round_trip_minimal() {
    let roadmap = sample_roadmap(vec![]);
    let yaml = serde_saphyr::to_string(&roadmap).unwrap();
    let parsed: Roadmap = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(roadmap, parsed);
}

#[test]
fn serde_round_trip_full() {
    let roadmap = Roadmap {
        title: "Enterprise Roadmap".into(),
        preamble: "The transformation begins.\n".into(),
        vision: "A platform for the future.\n".into(),
        principles: vec!["Engine, not application".into(), "Protocol-first".into()],
        phases: vec![
            Phase {
                id: "E1".into(),
                name: "Domain Model Extraction".into(),
                tagline: Some("The engine takes shape".into()),
                status: PhaseStatus::InProgress,
                problem: "Business logic scattered across TUI state.\n".into(),
                solution: "Extract a standalone domain layer.\n".into(),
                depends_on: vec![],
                tasks: vec![
                    Task {
                        text: "Define core types".into(),
                        checked: true,
                        detail: "Session, Iteration, WorkflowPhase\n".into(),
                        subtasks: vec![
                            sample_task("Session type", true),
                            sample_task("Iteration type", false),
                        ],
                    },
                    sample_task("Extract engine module", false),
                ],
            },
            Phase {
                id: "E2".into(),
                name: "Protocol Layer".into(),
                tagline: Some("The universal language".into()),
                status: PhaseStatus::Pending,
                problem: String::new(),
                solution: String::new(),
                depends_on: vec!["E1".into()],
                tasks: vec![],
            },
        ],
        epilogue: "## Success Criteria\n\n- Everything works\n".into(),
    };

    let yaml = serde_saphyr::to_string(&roadmap).unwrap();
    let parsed: Roadmap = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(roadmap, parsed);
}

#[test]
fn serde_defaults_for_missing_fields() {
    let yaml = "title: Minimal\nphases: []\n";
    let roadmap: Roadmap = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(roadmap.title, "Minimal");
    assert!(roadmap.preamble.is_empty());
    assert!(roadmap.vision.is_empty());
    assert!(roadmap.principles.is_empty());
    assert!(roadmap.epilogue.is_empty());
}

#[test]
fn serde_skip_empty_fields() {
    let roadmap = sample_roadmap(vec![]);
    let yaml = serde_saphyr::to_string(&roadmap).unwrap();
    assert!(!yaml.contains("preamble"));
    assert!(!yaml.contains("vision"));
    assert!(!yaml.contains("principles"));
    assert!(!yaml.contains("epilogue"));
}

#[test]
fn serde_task_defaults() {
    let yaml = "text: Do something\n";
    let task: Task = serde_saphyr::from_str(yaml).unwrap();
    assert_eq!(task.text, "Do something");
    assert!(!task.checked);
    assert!(task.detail.is_empty());
    assert!(task.subtasks.is_empty());
}

#[test]
fn serde_phase_status_values() {
    for (yaml, expected) in [
        ("pending", PhaseStatus::Pending),
        ("in_progress", PhaseStatus::InProgress),
        ("done", PhaseStatus::Done),
        ("blocked", PhaseStatus::Blocked),
    ] {
        let parsed: PhaseStatus = serde_saphyr::from_str(&format!("\"{yaml}\"")).unwrap();
        assert_eq!(parsed, expected);
    }
}

#[test]
fn serde_block_scalars_preserved() {
    let yaml = r#"
title: Test
phases:
  - id: E1
    name: Test Phase
    problem: |
      This is a multi-line
      problem statement.
    tasks:
      - text: A task
        detail: |
          Some detail
          on multiple lines.
"#;
    let roadmap: Roadmap = serde_saphyr::from_str(yaml).unwrap();
    assert!(roadmap.phases[0].problem.contains("multi-line"));
    assert!(roadmap.phases[0].tasks[0].detail.contains("multiple lines"));
}

#[test]
fn serde_unicode_content() {
    let roadmap = Roadmap {
        title: "Roadmap — Enterprise ✦ 日本語".into(),
        phases: vec![sample_phase(
            "E1",
            "Phase — One ✦",
            vec![sample_task("Task with émojis 🎯", false)],
        )],
        ..sample_roadmap(vec![])
    };
    let yaml = serde_saphyr::to_string(&roadmap).unwrap();
    let parsed: Roadmap = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(roadmap, parsed);
}

// ── Validation ──────────────────────────────────────────────────────────

#[test]
fn validate_valid_roadmap() {
    let roadmap = sample_roadmap(vec![
        sample_phase("E1", "Phase One", vec![]),
        sample_phase("E2", "Phase Two", vec![]),
    ]);
    assert!(roadmap.validate().is_ok());
}

#[test]
fn validate_empty_title() {
    let roadmap = Roadmap {
        title: "  ".into(),
        ..sample_roadmap(vec![])
    };
    assert!(roadmap.validate().is_err());
}

#[test]
fn validate_empty_phase_id() {
    let roadmap = sample_roadmap(vec![sample_phase("", "No ID", vec![])]);
    assert!(roadmap.validate().is_err());
}

#[test]
fn validate_duplicate_phase_ids() {
    let roadmap = sample_roadmap(vec![
        sample_phase("E1", "First", vec![]),
        sample_phase("E1", "Duplicate", vec![]),
    ]);
    let err = roadmap.validate().unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn validate_unknown_dependency() {
    let mut roadmap = sample_roadmap(vec![sample_phase("E1", "One", vec![])]);
    roadmap.phases[0].depends_on = vec!["E99".into()];
    let err = roadmap.validate().unwrap_err();
    assert!(err.to_string().contains("unknown"));
}

#[test]
fn validate_self_dependency() {
    let mut roadmap = sample_roadmap(vec![sample_phase("E1", "One", vec![])]);
    roadmap.phases[0].depends_on = vec!["E1".into()];
    let err = roadmap.validate().unwrap_err();
    assert!(err.to_string().contains("itself"));
}

#[test]
fn validate_circular_dependency() {
    let mut p1 = sample_phase("E1", "One", vec![]);
    let mut p2 = sample_phase("E2", "Two", vec![]);
    p1.depends_on = vec!["E2".into()];
    p2.depends_on = vec!["E1".into()];
    let roadmap = sample_roadmap(vec![p1, p2]);
    let err = roadmap.validate().unwrap_err();
    assert!(err.to_string().contains("circular"));
}

#[test]
fn validate_valid_dependency_chain() {
    let mut p2 = sample_phase("E2", "Two", vec![]);
    let mut p3 = sample_phase("E3", "Three", vec![]);
    p2.depends_on = vec!["E1".into()];
    p3.depends_on = vec!["E2".into()];
    let roadmap = sample_roadmap(vec![sample_phase("E1", "One", vec![]), p2, p3]);
    assert!(roadmap.validate().is_ok());
}

// ── Counting ────────────────────────────────────────────────────────────

#[test]
fn task_total_count_flat() {
    let task = sample_task("A", false);
    assert_eq!(task.total_count(), 1);
}

#[test]
fn task_total_count_with_subtasks() {
    let task = Task {
        text: "Parent".into(),
        checked: false,
        detail: String::new(),
        subtasks: vec![sample_task("Child 1", true), sample_task("Child 2", false)],
    };
    assert_eq!(task.total_count(), 3);
}

#[test]
fn task_checked_count() {
    let task = Task {
        text: "Parent".into(),
        checked: true,
        detail: String::new(),
        subtasks: vec![sample_task("Child 1", true), sample_task("Child 2", false)],
    };
    assert_eq!(task.checked_count(), 2);
}

#[test]
fn phase_task_counts() {
    let phase = sample_phase(
        "E1",
        "Test",
        vec![
            sample_task("A", true),
            Task {
                text: "B".into(),
                checked: false,
                detail: String::new(),
                subtasks: vec![sample_task("B1", true)],
            },
        ],
    );
    assert_eq!(phase.total_tasks(), 3);
    assert_eq!(phase.checked_tasks(), 2);
}

#[test]
fn roadmap_task_counts() {
    let roadmap = sample_roadmap(vec![
        sample_phase("E1", "One", vec![sample_task("A", true)]),
        sample_phase("E2", "Two", vec![sample_task("B", false)]),
    ]);
    assert_eq!(roadmap.total_tasks(), 2);
    assert_eq!(roadmap.checked_tasks(), 1);
}

// ── Status recomputation ────────────────────────────────────────────────

#[test]
fn recompute_status_all_done() {
    let mut phase = sample_phase(
        "E1",
        "Test",
        vec![sample_task("A", true), sample_task("B", true)],
    );
    phase.recompute_status();
    assert_eq!(phase.status, PhaseStatus::Done);
}

#[test]
fn recompute_status_partial() {
    let mut phase = sample_phase(
        "E1",
        "Test",
        vec![sample_task("A", true), sample_task("B", false)],
    );
    phase.recompute_status();
    assert_eq!(phase.status, PhaseStatus::InProgress);
}

#[test]
fn recompute_status_none_done() {
    let mut phase = sample_phase(
        "E1",
        "Test",
        vec![sample_task("A", false), sample_task("B", false)],
    );
    phase.recompute_status();
    assert_eq!(phase.status, PhaseStatus::Pending);
}

#[test]
fn recompute_status_no_tasks_keeps_current() {
    let mut phase = sample_phase("E1", "Test", vec![]);
    phase.status = PhaseStatus::InProgress;
    phase.recompute_status();
    assert_eq!(phase.status, PhaseStatus::InProgress);
}

#[test]
fn recompute_status_blocked_not_overridden() {
    let mut phase = sample_phase(
        "E1",
        "Test",
        vec![sample_task("A", true), sample_task("B", true)],
    );
    phase.status = PhaseStatus::Blocked;
    phase.recompute_status();
    assert_eq!(phase.status, PhaseStatus::Blocked);
}

#[test]
fn recompute_roadmap_statuses() {
    let mut roadmap = sample_roadmap(vec![
        sample_phase("E1", "One", vec![sample_task("A", true)]),
        sample_phase("E2", "Two", vec![sample_task("B", false)]),
    ]);
    roadmap.recompute_statuses();
    assert_eq!(roadmap.phases[0].status, PhaseStatus::Done);
    assert_eq!(roadmap.phases[1].status, PhaseStatus::Pending);
}

// ── PhaseStatus display ─────────────────────────────────────────────────

#[test]
fn phase_status_labels() {
    assert_eq!(PhaseStatus::Pending.label(), "pending");
    assert_eq!(PhaseStatus::InProgress.label(), "in_progress");
    assert_eq!(PhaseStatus::Done.label(), "done");
    assert_eq!(PhaseStatus::Blocked.label(), "blocked");
}

#[test]
fn phase_status_display() {
    assert_eq!(format!("{}", PhaseStatus::Done), "done");
}

#[test]
fn phase_status_default_is_pending() {
    assert_eq!(PhaseStatus::default(), PhaseStatus::Pending);
}
