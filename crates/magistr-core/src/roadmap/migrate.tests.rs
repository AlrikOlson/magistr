use super::*;

#[test]
fn parse_minimal_roadmap() {
    let md = "# My Project\n\n## E1: First Phase\n\n### Tasks\n\n- [ ] Do something\n";
    let roadmap = parse_markdown(md).unwrap();
    assert_eq!(roadmap.title, "My Project");
    assert_eq!(roadmap.phases.len(), 1);
    assert_eq!(roadmap.phases[0].id, "E1");
    assert_eq!(roadmap.phases[0].name, "First Phase");
    assert_eq!(roadmap.phases[0].tasks.len(), 1);
    assert!(!roadmap.phases[0].tasks[0].checked);
    assert_eq!(roadmap.phases[0].tasks[0].text, "Do something");
}

#[test]
fn parse_full_roadmap() {
    let md = "\
# Test Roadmap

Some preamble text here.

**Vision:** Build amazing things.

**Principles:**
- Keep it simple
- Ship fast

---

## Phase E0: Setup ✦ \"Getting started\"

**Problem:** Nothing exists yet.

**Solution:** Scaffold everything.

### Tasks

- [x] Create project structure
  - [x] Add Cargo.toml
  - [x] Add src/main.rs
- [ ] Write README

---

## Phase E1: Features ✦ \"The good stuff\"

**Problem:** No features.

### Tasks

- [ ] Add feature A
- [x] Add feature B

---

Some epilogue text.
";
    let roadmap = parse_markdown(md).unwrap();

    assert_eq!(roadmap.title, "Test Roadmap");
    assert_eq!(roadmap.preamble, "Some preamble text here.");
    assert_eq!(roadmap.vision, "Build amazing things.");
    assert_eq!(roadmap.principles.len(), 2);
    assert_eq!(roadmap.principles[0], "Keep it simple");
    assert_eq!(roadmap.principles[1], "Ship fast");
    assert_eq!(roadmap.epilogue, "Some epilogue text.");

    // Phase E0
    assert_eq!(roadmap.phases.len(), 2);
    let e0 = &roadmap.phases[0];
    assert_eq!(e0.id, "E0");
    assert_eq!(e0.name, "Setup");
    assert_eq!(e0.tagline.as_deref(), Some("Getting started"));
    assert_eq!(e0.problem, "Nothing exists yet.");
    assert_eq!(e0.solution, "Scaffold everything.");
    assert_eq!(e0.tasks.len(), 2);
    assert!(e0.tasks[0].checked);
    assert_eq!(e0.tasks[0].text, "Create project structure");
    assert_eq!(e0.tasks[0].subtasks.len(), 2);
    assert!(e0.tasks[0].subtasks[0].checked);
    assert_eq!(e0.tasks[0].subtasks[0].text, "Add Cargo.toml");
    assert!(!e0.tasks[1].checked);
    assert_eq!(e0.tasks[1].text, "Write README");

    // Phase E1
    let e1 = &roadmap.phases[1];
    assert_eq!(e1.id, "E1");
    assert_eq!(e1.name, "Features");
    assert_eq!(e1.tagline.as_deref(), Some("The good stuff"));
    assert_eq!(e1.tasks.len(), 2);
    assert!(!e1.tasks[0].checked);
    assert!(e1.tasks[1].checked);
}

#[test]
fn parse_phase_header_with_tagline() {
    let phase = parse_phase_header("Phase E0: Setup ✦ \"Getting started\"");
    assert_eq!(phase.id, "E0");
    assert_eq!(phase.name, "Setup");
    assert_eq!(phase.tagline.as_deref(), Some("Getting started"));
}

#[test]
fn parse_phase_header_without_tagline() {
    let phase = parse_phase_header("Phase E1: Core Features");
    assert_eq!(phase.id, "E1");
    assert_eq!(phase.name, "Core Features");
    assert!(phase.tagline.is_none());
}

#[test]
fn parse_phase_header_no_phase_prefix() {
    let phase = parse_phase_header("E2: Advanced");
    assert_eq!(phase.id, "E2");
    assert_eq!(phase.name, "Advanced");
}

#[test]
fn parse_phase_header_no_colon_auto_generates_id() {
    let phase = parse_phase_header("Migration Path & Shipping Cadence");
    assert_eq!(phase.id, "MP&SC");
    assert_eq!(phase.name, "Migration Path & Shipping Cadence");
    assert!(phase.tagline.is_none());
}

#[test]
fn parse_phase_header_single_word_no_colon() {
    let phase = parse_phase_header("Overview");
    assert_eq!(phase.id, "O");
    assert_eq!(phase.name, "Overview");
}

#[test]
fn parse_roadmap_with_no_id_phases() {
    let md = "\
# Roadmap

## Migration Path & Shipping Cadence

### Tasks

- [ ] Plan migration
- [x] Ship v1
";
    let roadmap = parse_markdown(md).unwrap();
    assert_eq!(roadmap.phases.len(), 1);
    assert_eq!(roadmap.phases[0].id, "MP&SC");
    assert_eq!(roadmap.phases[0].name, "Migration Path & Shipping Cadence");
    assert_eq!(roadmap.phases[0].tasks.len(), 2);
}

#[test]
fn parse_task_line_checked() {
    let task = try_parse_task_line("- [x] Do the thing", 0).unwrap();
    assert!(task.checked);
    assert_eq!(task.text, "Do the thing");
}

#[test]
fn parse_task_line_unchecked() {
    let task = try_parse_task_line("- [ ] Not done", 0).unwrap();
    assert!(!task.checked);
    assert_eq!(task.text, "Not done");
}

#[test]
fn parse_task_line_indented() {
    let task = try_parse_task_line("  - [x] Subtask", 2).unwrap();
    assert!(task.checked);
    assert_eq!(task.text, "Subtask");
}

#[test]
fn parse_task_line_not_a_task() {
    assert!(try_parse_task_line("Just text", 0).is_none());
    assert!(try_parse_task_line("- No checkbox", 0).is_none());
}

#[test]
fn parse_empty_content() {
    let roadmap = parse_markdown("").unwrap();
    assert!(roadmap.title.is_empty());
    assert!(roadmap.phases.is_empty());
}

#[test]
fn parse_phases_without_tasks_section() {
    let md = "# Roadmap\n\n## E1: Phase One\n\n**Problem:** Something.\n";
    let roadmap = parse_markdown(md).unwrap();
    assert_eq!(roadmap.phases.len(), 1);
    assert!(roadmap.phases[0].tasks.is_empty());
    assert_eq!(roadmap.phases[0].problem, "Something.");
}

#[test]
fn parse_deeply_nested_subtasks() {
    let md = "\
# Roadmap

## E1: Phase

### Tasks

- [ ] Top level
  - [ ] Level 2
    - [x] Level 3
";
    let roadmap = parse_markdown(md).unwrap();
    let top = &roadmap.phases[0].tasks[0];
    assert_eq!(top.text, "Top level");
    assert_eq!(top.subtasks.len(), 1);
    assert_eq!(top.subtasks[0].text, "Level 2");
    assert_eq!(top.subtasks[0].subtasks.len(), 1);
    assert_eq!(top.subtasks[0].subtasks[0].text, "Level 3");
    assert!(top.subtasks[0].subtasks[0].checked);
}

#[test]
fn parse_status_recomputed() {
    let md = "\
# Roadmap

## E1: Done Phase

### Tasks

- [x] Task A
- [x] Task B

## E2: Partial Phase

### Tasks

- [x] Done
- [ ] Not done
";
    let roadmap = parse_markdown(md).unwrap();
    assert_eq!(roadmap.phases[0].status, PhaseStatus::Done);
    assert_eq!(roadmap.phases[1].status, PhaseStatus::InProgress);
}

#[test]
fn parse_multiline_problem() {
    let md = "\
# Roadmap

## E1: Phase

**Problem:** First line of problem
continues on this line.

### Tasks

- [ ] Task
";
    let roadmap = parse_markdown(md).unwrap();
    assert_eq!(
        roadmap.phases[0].problem,
        "First line of problem\ncontinues on this line."
    );
}

#[test]
fn migration_round_trip_with_yaml_store() {
    use crate::roadmap::yaml_store::YamlStore;
    use tempfile::TempDir;

    let md = "\
# Test Project

## E1: Setup

### Tasks

- [x] Init project
- [ ] Add CI
";
    let roadmap = parse_markdown(md).unwrap();
    let dir = TempDir::new().unwrap();
    let yaml_path = dir.path().join("roadmap.yaml");

    let mut store = YamlStore::from_roadmap(&yaml_path, roadmap).unwrap();
    store.save().unwrap();

    let loaded = YamlStore::load(&yaml_path).unwrap();
    assert_eq!(loaded.index().stats().total_tasks, 2);
    assert_eq!(loaded.index().stats().checked_tasks, 1);
}
