use super::*;
use tempfile::TempDir;

fn sample_plan(id: &str, status: PlanStatus) -> SavedPlan {
    SavedPlan {
        id: id.to_string(),
        created_at: Utc::now(),
        status,
        task_name: "Test plan".to_string(),
        content: "# Plan\n\n- Step 1\n- Step 2\n".to_string(),
        spec_items: vec!["item 1".to_string(), "item 2".to_string()],
    }
}

#[test]
fn save_and_load_round_trip() {
    let tmp = TempDir::new().unwrap();
    let plan = sample_plan("20260310-120000-test", PlanStatus::Active);

    let path = save_plan(tmp.path(), &plan).unwrap();
    assert!(path.exists());

    let loaded = load_plan(tmp.path(), "20260310-120000-test").unwrap();
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.status, PlanStatus::Active);
    assert_eq!(loaded.task_name, "Test plan");
    assert_eq!(loaded.content, plan.content);
    assert_eq!(loaded.spec_items, plan.spec_items);
}

#[test]
fn load_nonexistent_returns_error() {
    let tmp = TempDir::new().unwrap();
    let result = load_plan(tmp.path(), "nonexistent");
    assert!(result.is_err());
}

#[test]
fn list_empty_dir_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let plans = list_plans(tmp.path()).unwrap();
    assert!(plans.is_empty());
}

#[test]
fn list_returns_plans_sorted_by_date() {
    let tmp = TempDir::new().unwrap();

    let mut older = sample_plan("20260309-100000-first", PlanStatus::Completed);
    older.created_at = DateTime::parse_from_rfc3339("2026-03-09T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let mut newer = sample_plan("20260310-100000-second", PlanStatus::Active);
    newer.created_at = DateTime::parse_from_rfc3339("2026-03-10T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    save_plan(tmp.path(), &older).unwrap();
    save_plan(tmp.path(), &newer).unwrap();

    let plans = list_plans(tmp.path()).unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].id, "20260310-100000-second"); // newer first
    assert_eq!(plans[1].id, "20260309-100000-first");
}

#[test]
fn find_active_plan_returns_most_recent() {
    let tmp = TempDir::new().unwrap();

    let completed = sample_plan("20260309-100000-done", PlanStatus::Completed);
    let active = sample_plan("20260310-100000-active", PlanStatus::Active);

    save_plan(tmp.path(), &completed).unwrap();
    save_plan(tmp.path(), &active).unwrap();

    let found = find_active_plan(tmp.path()).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "20260310-100000-active");
}

#[test]
fn find_active_plan_returns_none_when_no_active() {
    let tmp = TempDir::new().unwrap();

    let completed = sample_plan("20260309-100000-done", PlanStatus::Completed);
    save_plan(tmp.path(), &completed).unwrap();

    let found = find_active_plan(tmp.path()).unwrap();
    assert!(found.is_none());
}

#[test]
fn update_status_changes_plan() {
    let tmp = TempDir::new().unwrap();

    let plan = sample_plan("20260310-test", PlanStatus::Active);
    save_plan(tmp.path(), &plan).unwrap();

    update_status(tmp.path(), "20260310-test", PlanStatus::Completed).unwrap();

    let loaded = load_plan(tmp.path(), "20260310-test").unwrap();
    assert_eq!(loaded.status, PlanStatus::Completed);
}

#[test]
fn abandon_active_plans_marks_all_active_as_abandoned() {
    let tmp = TempDir::new().unwrap();

    let active1 = sample_plan("20260309-a", PlanStatus::Active);
    let active2 = sample_plan("20260310-b", PlanStatus::Active);
    let completed = sample_plan("20260308-c", PlanStatus::Completed);

    save_plan(tmp.path(), &active1).unwrap();
    save_plan(tmp.path(), &active2).unwrap();
    save_plan(tmp.path(), &completed).unwrap();

    let count = abandon_active_plans(tmp.path()).unwrap();
    assert_eq!(count, 2);

    let plans = list_plans(tmp.path()).unwrap();
    for p in &plans {
        assert_ne!(p.status, PlanStatus::Active);
    }
    // Completed plan stays completed
    let c = plans.iter().find(|p| p.id == "20260308-c").unwrap();
    assert_eq!(c.status, PlanStatus::Completed);
}

#[test]
fn generate_id_includes_timestamp_and_slug() {
    let id = generate_id("Fix authentication bug");
    assert!(id.contains("fix-authentication-bug"));
    // Should start with a date pattern
    assert!(id.chars().next().unwrap().is_ascii_digit());
}

#[test]
fn slugify_basic() {
    assert_eq!(slugify("Hello World"), "hello-world");
}

#[test]
fn slugify_special_chars() {
    assert_eq!(slugify("Fix: auth & login!"), "fix-auth-login");
}

#[test]
fn slugify_long_name_truncated() {
    let long = "a".repeat(60);
    let slug = slugify(&long);
    assert!(slug.len() <= 40);
}

#[test]
fn slugify_no_leading_or_trailing_dashes() {
    assert_eq!(slugify("  hello  "), "hello");
    assert_eq!(slugify("--hello--"), "hello");
}

#[test]
fn plan_status_display() {
    assert_eq!(format!("{}", PlanStatus::Active), "active");
    assert_eq!(format!("{}", PlanStatus::Completed), "completed");
    assert_eq!(format!("{}", PlanStatus::Abandoned), "abandoned");
}

#[test]
fn plan_status_icons() {
    assert_eq!(PlanStatus::Active.icon(), "●");
    assert_eq!(PlanStatus::Completed.icon(), "✓");
    assert_eq!(PlanStatus::Abandoned.icon(), "✗");
}

#[test]
fn archive_current_plan_creates_entry() {
    let tmp = TempDir::new().unwrap();

    // Create the project directory and plan files
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("current-plan.md"),
        "# My Plan\n\n- Do stuff\n",
    )
    .unwrap();
    std::fs::write(magistr_dir.join("current-task.md"), "Fix the widget\n").unwrap();

    let plan = archive_current_plan(tmp.path()).unwrap();
    assert_eq!(plan.status, PlanStatus::Active);
    assert_eq!(plan.task_name, "Fix the widget");
    assert!(plan.content.contains("# My Plan"));

    // Should be loadable from disk
    let loaded = load_plan(tmp.path(), &plan.id).unwrap();
    assert_eq!(loaded.task_name, "Fix the widget");
}

#[test]
fn archive_current_plan_abandons_previous_active() {
    let tmp = TempDir::new().unwrap();

    // Create an existing active plan
    let existing = sample_plan("20260309-old", PlanStatus::Active);
    save_plan(tmp.path(), &existing).unwrap();

    // Create plan files
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("current-plan.md"), "# New Plan\n").unwrap();
    std::fs::write(magistr_dir.join("current-task.md"), "New task\n").unwrap();

    archive_current_plan(tmp.path()).unwrap();

    // Old plan should now be abandoned
    let old = load_plan(tmp.path(), "20260309-old").unwrap();
    assert_eq!(old.status, PlanStatus::Abandoned);
}

#[test]
fn archive_current_plan_missing_plan_file_errors() {
    let tmp = TempDir::new().unwrap();
    let result = archive_current_plan(tmp.path());
    assert!(result.is_err());
}

#[test]
fn list_skips_malformed_files() {
    let tmp = TempDir::new().unwrap();
    let dir = plans_dir(tmp.path());
    std::fs::create_dir_all(&dir).unwrap();

    // Write a valid plan
    let plan = sample_plan("valid", PlanStatus::Active);
    save_plan(tmp.path(), &plan).unwrap();

    // Write a malformed TOML file
    std::fs::write(dir.join("bad.toml"), "this is not valid toml {{{").unwrap();

    let plans = list_plans(tmp.path()).unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].id, "valid");
}

#[test]
fn save_creates_plans_directory() {
    let tmp = TempDir::new().unwrap();
    let plan = sample_plan("test", PlanStatus::Active);

    assert!(!plans_dir(tmp.path()).exists());
    save_plan(tmp.path(), &plan).unwrap();
    assert!(plans_dir(tmp.path()).exists());
}
