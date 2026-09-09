use super::*;
use crate::roadmap::migrate::parse_markdown;
use crate::roadmap::{PhaseStatus, Task};

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
    let mut p = Phase {
        id: id.into(),
        name: name.into(),
        tagline: None,
        status: PhaseStatus::default(),
        problem: String::new(),
        solution: String::new(),
        depends_on: vec![],
        tasks,
    };
    p.recompute_status();
    p
}

fn minimal_roadmap() -> Roadmap {
    Roadmap {
        title: "My Project".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![phase(
            "E1",
            "First Phase",
            vec![task("Do something", false)],
        )],
        epilogue: String::new(),
    }
}

#[test]
fn render_minimal() {
    let md = render(&minimal_roadmap());
    assert!(md.starts_with("# My Project\n"));
    assert!(md.contains("## Phase E1: First Phase"));
    assert!(md.contains("- [ ] Do something"));
}

#[test]
fn render_with_tagline() {
    let mut roadmap = minimal_roadmap();
    roadmap.phases[0].tagline = Some("Getting started".into());
    let md = render(&roadmap);
    assert!(md.contains("## Phase E1: First Phase ✦ \"Getting started\""));
}

#[test]
fn render_checked_and_unchecked() {
    let roadmap = Roadmap {
        title: "Test".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![phase(
            "E1",
            "Phase",
            vec![task("Done", true), task("Todo", false)],
        )],
        epilogue: String::new(),
    };
    let md = render(&roadmap);
    assert!(md.contains("- [x] Done"));
    assert!(md.contains("- [ ] Todo"));
}

#[test]
fn render_subtasks() {
    let roadmap = Roadmap {
        title: "Test".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![phase(
            "E1",
            "Phase",
            vec![task_with_subs(
                "Parent",
                false,
                vec![task("Child A", true), task("Child B", false)],
            )],
        )],
        epilogue: String::new(),
    };
    let md = render(&roadmap);
    assert!(md.contains("- [ ] Parent\n  - [x] Child A\n  - [ ] Child B"));
}

#[test]
fn render_deep_nesting() {
    let roadmap = Roadmap {
        title: "Test".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![phase(
            "E1",
            "Phase",
            vec![task_with_subs(
                "L1",
                false,
                vec![task_with_subs("L2", false, vec![task("L3", true)])],
            )],
        )],
        epilogue: String::new(),
    };
    let md = render(&roadmap);
    assert!(md.contains("- [ ] L1\n  - [ ] L2\n    - [x] L3"));
}

#[test]
fn render_problem_and_solution() {
    let mut roadmap = minimal_roadmap();
    roadmap.phases[0].problem = "Things are broken.".into();
    roadmap.phases[0].solution = "Fix them.".into();
    let md = render(&roadmap);
    assert!(md.contains("**Problem:** Things are broken."));
    assert!(md.contains("**Solution:** Fix them."));
}

#[test]
fn render_preamble_vision_principles() {
    let roadmap = Roadmap {
        title: "Test".into(),
        preamble: "Some intro text.".into(),
        vision: "Build great things.".into(),
        principles: vec!["Keep it simple".into(), "Ship fast".into()],
        phases: vec![phase("E1", "Phase", vec![])],
        epilogue: String::new(),
    };
    let md = render(&roadmap);
    assert!(md.contains("Some intro text."));
    assert!(md.contains("**Vision:** Build great things."));
    assert!(md.contains("**Principles:**\n- Keep it simple\n- Ship fast"));
}

#[test]
fn render_epilogue() {
    let mut roadmap = minimal_roadmap();
    roadmap.epilogue = "Final notes here.".into();
    let md = render(&roadmap);
    assert!(md.ends_with("---\n\nFinal notes here.\n"));
}

#[test]
fn render_multiple_phases_separated() {
    let roadmap = Roadmap {
        title: "Test".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![
            phase("E1", "First", vec![task("A", false)]),
            phase("E2", "Second", vec![task("B", true)]),
        ],
        epilogue: String::new(),
    };
    let md = render(&roadmap);
    assert!(md.contains("---\n\n## Phase E2:"));
}

#[test]
fn round_trip_minimal() {
    let original = minimal_roadmap();
    let md = render(&original);
    let parsed = parse_markdown(&md).unwrap();
    assert_eq!(parsed.title, original.title);
    assert_eq!(parsed.phases.len(), original.phases.len());
    assert_eq!(parsed.phases[0].id, original.phases[0].id);
    assert_eq!(parsed.phases[0].name, original.phases[0].name);
    assert_eq!(parsed.phases[0].tasks.len(), original.phases[0].tasks.len());
    assert_eq!(
        parsed.phases[0].tasks[0].text,
        original.phases[0].tasks[0].text
    );
    assert_eq!(
        parsed.phases[0].tasks[0].checked,
        original.phases[0].tasks[0].checked
    );
}

#[test]
fn round_trip_full() {
    let roadmap = Roadmap {
        title: "Test Roadmap".into(),
        preamble: "Some preamble text here.".into(),
        vision: "Build amazing things.".into(),
        principles: vec!["Keep it simple".into(), "Ship fast".into()],
        phases: vec![
            {
                let mut p = phase(
                    "E0",
                    "Setup",
                    vec![
                        task_with_subs(
                            "Create project structure",
                            true,
                            vec![task("Add Cargo.toml", true), task("Add src/main.rs", true)],
                        ),
                        task("Write README", false),
                    ],
                );
                p.tagline = Some("Getting started".into());
                p.problem = "Nothing exists yet.".into();
                p.solution = "Scaffold everything.".into();
                p
            },
            {
                let mut p = phase(
                    "E1",
                    "Features",
                    vec![task("Add feature A", false), task("Add feature B", true)],
                );
                p.tagline = Some("The good stuff".into());
                p.problem = "No features.".into();
                p
            },
        ],
        epilogue: "Some epilogue text.".into(),
    };

    let md = render(&roadmap);
    let parsed = parse_markdown(&md).unwrap();

    assert_eq!(parsed.title, roadmap.title);
    assert_eq!(parsed.preamble, roadmap.preamble);
    assert_eq!(parsed.vision, roadmap.vision);
    assert_eq!(parsed.principles, roadmap.principles);
    assert_eq!(parsed.epilogue, roadmap.epilogue);
    assert_eq!(parsed.phases.len(), 2);

    // E0
    assert_eq!(parsed.phases[0].id, "E0");
    assert_eq!(parsed.phases[0].name, "Setup");
    assert_eq!(parsed.phases[0].tagline.as_deref(), Some("Getting started"));
    assert_eq!(parsed.phases[0].problem, "Nothing exists yet.");
    assert_eq!(parsed.phases[0].solution, "Scaffold everything.");
    assert_eq!(parsed.phases[0].tasks.len(), 2);
    assert!(parsed.phases[0].tasks[0].checked);
    assert_eq!(parsed.phases[0].tasks[0].subtasks.len(), 2);

    // E1
    assert_eq!(parsed.phases[1].id, "E1");
    assert_eq!(parsed.phases[1].tagline.as_deref(), Some("The good stuff"));
    assert_eq!(parsed.phases[1].tasks.len(), 2);
}
