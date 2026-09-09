use super::*;
use std::io::Write;
use tempfile::TempDir;

fn write_toml(dir: &std::path::Path, name: &str, content: &str) {
    let agents_dir = dir.join(".magistr").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();
    let mut f = std::fs::File::create(agents_dir.join(name)).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

#[test]
fn load_empty_dir_returns_empty() {
    let dir = TempDir::new().unwrap();
    let roles = load_custom_roles(dir.path());
    assert!(roles.is_empty());
}

#[test]
fn load_missing_dir_returns_empty() {
    let dir = TempDir::new().unwrap();
    let nonexistent = dir.path().join("nope");
    let roles = load_custom_roles(&nonexistent);
    assert!(roles.is_empty());
}

#[test]
fn load_valid_toml() {
    let dir = TempDir::new().unwrap();
    write_toml(
        dir.path(),
        "security-auditor.toml",
        r#"
role_id = "security-auditor"
display_name = "Security Auditor"
system_prompt_template = "Audit for vulnerabilities.\n\nTask: {task}"
allowed_tools = ["Read", "Bash"]
denied_tools = ["Edit", "Write"]
icon = "🛡"
color_index = 5

[constraints]
read_only = true
max_iterations = 10

[model]
preference = "claude-opus-4-6"
fallback = ["claude-sonnet-4-6"]
"#,
    );

    let roles = load_custom_roles(dir.path());
    assert_eq!(roles.len(), 1);
    let role = &roles[0];
    assert_eq!(role.role_id, "security-auditor");
    assert_eq!(role.display_name, "Security Auditor");
    assert!(role.constraints.read_only);
    assert_eq!(role.constraints.max_iterations, 10);
    assert_eq!(role.model_preference.as_deref(), Some("claude-opus-4-6"));
    assert!(role.denied_tools.contains(&"Edit".to_string()));
    assert_eq!(role.icon, '\u{1f6e1}'); // 🛡
}

#[test]
fn load_minimal_toml() {
    let dir = TempDir::new().unwrap();
    write_toml(
        dir.path(),
        "planner.toml",
        r#"
role_id = "planner"
display_name = "Planner"
system_prompt_template = "Plan the implementation.\n\nTask: {task}"
"#,
    );

    let roles = load_custom_roles(dir.path());
    assert_eq!(roles.len(), 1);
    let role = &roles[0];
    assert_eq!(role.role_id, "planner");
    assert!(!role.constraints.read_only);
    assert!(role.model_preference.is_none());
    assert!(role.allowed_tools.is_empty());
    assert!(role.denied_tools.is_empty());
}

#[test]
fn skip_non_toml_files() {
    let dir = TempDir::new().unwrap();
    let agents_dir = dir.path().join(".magistr").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();
    std::fs::write(agents_dir.join("readme.md"), "# Custom Agents").unwrap();
    write_toml(
        dir.path(),
        "valid.toml",
        r#"
role_id = "valid"
display_name = "Valid"
system_prompt_template = "Do the thing.\n\nTask: {task}"
"#,
    );

    let roles = load_custom_roles(dir.path());
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].role_id, "valid");
}

#[test]
fn skip_invalid_toml() {
    let dir = TempDir::new().unwrap();
    write_toml(dir.path(), "broken.toml", "this is not valid toml {{{{");
    write_toml(
        dir.path(),
        "good.toml",
        r#"
role_id = "good"
display_name = "Good"
system_prompt_template = "Task: {task}"
"#,
    );

    let roles = load_custom_roles(dir.path());
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].role_id, "good");
}

#[test]
fn merge_custom_with_built_ins() {
    let dir = TempDir::new().unwrap();
    write_toml(
        dir.path(),
        "custom.toml",
        r#"
role_id = "custom-role"
display_name = "Custom"
system_prompt_template = "Task: {task}"
"#,
    );

    let all = load_all_roles(dir.path());
    // 5 built-in + 1 custom
    assert_eq!(all.len(), 6);
    assert!(all.iter().any(|r| r.role_id == "custom-role"));
    assert!(all.iter().any(|r| r.role_id == "researcher"));
}

#[test]
fn custom_overrides_built_in_by_id() {
    let dir = TempDir::new().unwrap();
    write_toml(
        dir.path(),
        "researcher.toml",
        r#"
role_id = "researcher"
display_name = "Custom Researcher"
system_prompt_template = "Custom research prompt.\n\nTask: {task}"
"#,
    );

    let all = load_all_roles(dir.path());
    // Still 5 total — custom replaces built-in
    assert_eq!(all.len(), 5);
    let researcher = all.iter().find(|r| r.role_id == "researcher").unwrap();
    assert_eq!(researcher.display_name, "Custom Researcher");
}
