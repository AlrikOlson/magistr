use super::*;
use crate::plan::{PhaseStatus, Task};

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

fn minimal_plan() -> Plan {
    Plan {
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
    let md = render(&minimal_plan());
    assert!(md.starts_with("# My Project\n"));
    assert!(md.contains("## Phase E1: First Phase"));
    assert!(md.contains("- [ ] Do something"));
}

#[test]
fn render_with_tagline() {
    let mut plan = minimal_plan();
    plan.phases[0].tagline = Some("Getting started".into());
    let md = render(&plan);
    assert!(md.contains("## Phase E1: First Phase ✦ \"Getting started\""));
}

#[test]
fn render_checked_and_unchecked() {
    let plan = Plan {
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
    let md = render(&plan);
    assert!(md.contains("- [x] Done"));
    assert!(md.contains("- [ ] Todo"));
}

#[test]
fn render_subtasks() {
    let plan = Plan {
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
    let md = render(&plan);
    assert!(md.contains("- [ ] Parent\n  - [x] Child A\n  - [ ] Child B"));
}

#[test]
fn render_deep_nesting() {
    let plan = Plan {
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
    let md = render(&plan);
    assert!(md.contains("- [ ] L1\n  - [ ] L2\n    - [x] L3"));
}

#[test]
fn render_problem_and_solution() {
    let mut plan = minimal_plan();
    plan.phases[0].problem = "Things are broken.".into();
    plan.phases[0].solution = "Fix them.".into();
    let md = render(&plan);
    assert!(md.contains("**Problem:** Things are broken."));
    assert!(md.contains("**Solution:** Fix them."));
}

#[test]
fn render_preamble_vision_principles() {
    let plan = Plan {
        title: "Test".into(),
        preamble: "Some intro text.".into(),
        vision: "Build great things.".into(),
        principles: vec!["Keep it simple".into(), "Run fast".into()],
        phases: vec![phase("E1", "Phase", vec![])],
        epilogue: String::new(),
    };
    let md = render(&plan);
    assert!(md.contains("Some intro text."));
    assert!(md.contains("**Vision:** Build great things."));
    assert!(md.contains("**Principles:**\n- Keep it simple\n- Run fast"));
}

#[test]
fn render_epilogue() {
    let mut plan = minimal_plan();
    plan.epilogue = "Final notes here.".into();
    let md = render(&plan);
    assert!(md.ends_with("---\n\nFinal notes here.\n"));
}

#[test]
fn render_multiple_phases_separated() {
    let plan = Plan {
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
    let md = render(&plan);
    assert!(md.contains("---\n\n## Phase E2:"));
}
