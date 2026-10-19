use super::*;

/// Write what the seed plans, as setup does through the administrative
/// service; answers whether anything was written.
fn apply(seed: &WorkspaceSeed, root: &Path) -> std::io::Result<bool> {
    match seed.plan(root).1 {
        Some(body) => {
            std::fs::create_dir_all(root.join(".magistr")).unwrap();
            std::fs::write(root.join(".magistr/workspace.toml"), body)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn vars() -> HashMap<String, String> {
    HashMap::from([
        (
            "check_cmd".to_string(),
            "cargo check --all-targets".to_string(),
        ),
        ("test_cmd".to_string(), "cargo nextest run".to_string()),
        (
            "lint_cmd".to_string(),
            "cargo clippy -- -D warnings".to_string(),
        ),
        ("format_cmd".to_string(), "cargo fmt".to_string()),
    ])
}

#[test]
fn seed_picks_the_four_gate_keys_and_skips_blanks() {
    let mut v = vars();
    v.insert("lint_cmd".into(), "   ".into());
    let seed = WorkspaceSeed::from_template_vars(&v);
    let keys: Vec<&str> = seed.commands.iter().map(|(k, _, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["check", "test", "format"]);
    assert!(WorkspaceSeed::from_template_vars(&HashMap::new()).is_empty());
}

#[test]
fn render_is_a_valid_commands_table() {
    let rendered = WorkspaceSeed::from_template_vars(&vars()).render();
    let parsed: toml::Value = toml::from_str(&rendered).expect("valid toml");
    assert_eq!(
        parsed["commands"]["check"].as_str(),
        Some("cargo check --all-targets")
    );
    assert_eq!(parsed["commands"]["format"].as_str(), Some("cargo fmt"));
    assert!(rendered.contains("# project.json quality_gate.test"));
}

#[test]
fn quotes_and_backslashes_are_escaped() {
    let v = HashMap::from([("check_cmd".to_string(), r#"echo "a\b""#.to_string())]);
    let rendered = WorkspaceSeed::from_template_vars(&v).render();
    let parsed: toml::Value = toml::from_str(&rendered).expect("valid toml");
    assert_eq!(parsed["commands"]["check"].as_str(), Some(r#"echo "a\b""#));
}

#[test]
fn merge_appends_missing_keys_to_a_seeded_table() {
    let existing = "# Seeded by `magistr init`\n[commands]\n\"cargo-test\" = \"cargo test\" # Cargo.toml\n\"test\" = \"cargo nextest run\" # hand\n";
    let merged = WorkspaceSeed::from_template_vars(&vars())
        .merge_into(existing)
        .expect("something to add");
    let parsed: toml::Value = toml::from_str(&merged).expect("valid toml");
    assert_eq!(
        parsed["commands"]["cargo-test"].as_str(),
        Some("cargo test")
    );
    assert_eq!(
        parsed["commands"]["test"].as_str(),
        Some("cargo nextest run"),
        "existing key kept as the human wrote it"
    );
    assert_eq!(
        parsed["commands"]["check"].as_str(),
        Some("cargo check --all-targets")
    );
    assert_eq!(
        parsed["commands"]["lint"].as_str(),
        Some("cargo clippy -- -D warnings")
    );
}

#[test]
fn merge_is_a_noop_when_everything_is_declared() {
    let existing = WorkspaceSeed::from_template_vars(&vars()).render();
    assert!(
        WorkspaceSeed::from_template_vars(&vars())
            .merge_into(&existing)
            .is_none()
    );
}

#[test]
fn merge_refuses_when_commands_is_not_the_last_table() {
    let existing = "[commands]\n\"a\" = \"b\"\n\n[budgets]\ndefault = 4000\n";
    assert!(
        WorkspaceSeed::from_template_vars(&vars())
            .merge_into(existing)
            .is_none()
    );
}

#[test]
fn merge_adds_a_table_to_a_file_without_one() {
    let existing = "[budgets]\ndefault = 4000\n";
    let merged = WorkspaceSeed::from_template_vars(&vars())
        .merge_into(existing)
        .expect("table added");
    let parsed: toml::Value = toml::from_str(&merged).expect("valid toml");
    assert_eq!(parsed["budgets"]["default"].as_integer(), Some(4000));
    assert_eq!(
        parsed["commands"]["check"].as_str(),
        Some("cargo check --all-targets")
    );
}

#[test]
fn apply_creates_the_file_and_never_rewrites_a_declared_table() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let seed = WorkspaceSeed::from_template_vars(&vars());
    assert!(apply(&seed, tmp.path()).expect("apply"));
    std::fs::create_dir_all(tmp.path().join(".magistr")).unwrap();
    let path = tmp.path().join(".magistr/workspace.toml");
    let first = std::fs::read_to_string(&path).expect("written");
    assert!(first.contains("[commands]"));

    // A human edits the table; a re-init must leave it alone. The confirm
    // mode is a *different* key and is still declared, because the file the
    // human wrote does not say what it should be — see
    // `an_existing_commands_table_still_gets_the_confirm_mode`.
    std::fs::write(&path, "[commands]\n\"test\" = \"my own\"\n").expect("edit");
    assert!(apply(&seed, tmp.path()).expect("apply again"));
    let second = std::fs::read_to_string(&path).expect("read");
    assert!(
        second.starts_with("[commands]\n\"test\" = \"my own\"\n"),
        "the human's table is byte-identical: {second}"
    );
    assert!(
        !second.contains("cargo check"),
        "no gate row was added back"
    );
    // And once the confirm mode is there, a re-init writes nothing at all.
    assert!(!apply(&seed, tmp.path()).expect("apply a third time"));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), second);
}

#[test]
fn apply_with_nothing_to_seed_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let seed = WorkspaceSeed::default();
    assert!(!apply(&seed, tmp.path()).expect("apply"));
    assert!(!tmp.path().join(".magistr/workspace.toml").exists());
}

#[test]
fn rendered_table_snapshot() {
    insta::assert_snapshot!(
        "workspace_toml",
        WorkspaceSeed::from_template_vars(&vars()).render()
    );
}

fn project_config() -> magistr_core::types::ProjectConfig {
    serde_json::from_value(serde_json::json!({
        "project": {"name": "t", "description": "", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                  "test_framework": "cargo-nextest", "linter": "clippy", "formatter": "rustfmt"},
        "quality_gate": {"check": "cargo check --all-targets", "test": "cargo nextest run",
                         "lint": "cargo clippy -- -D warnings"},
        "custom_gates": [{"name": "structure", "command": "./scripts/lint-structure.sh", "required": true}],
        "format": {"command": "cargo fmt", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^- \\\\[ \\\\]"},
        "state_prefix": "t",
        "reference_repos": [],
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"}
    }))
    .expect("config")
}

#[test]
fn seed_from_project_config_includes_custom_gates() {
    let seed = WorkspaceSeed::from_project_config(&project_config());
    let keys: Vec<&str> = seed.commands.iter().map(|(k, _, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["check", "test", "lint", "structure", "format"]);
}

#[test]
fn drift_is_empty_when_the_table_matches() {
    let seed = WorkspaceSeed::from_project_config(&project_config());
    let body = seed.render();
    assert!(seed.drift(&body).is_empty(), "{body}");
}

#[test]
fn drift_reports_missing_keys_and_changed_commands() {
    let seed = WorkspaceSeed::from_project_config(&project_config());
    let body = "[commands]\n\
        \"check\" = \"cargo check\" # human edit\n\
        \"test\" = \"cargo nextest run\"\n\
        \"lint\" = \"cargo clippy -- -D warnings\" # project.json quality_gate.lint\n\
        \"format\" = \"cargo fmt\"\n";
    let findings = seed.drift(body);
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert!(
        findings[0].contains("`check` drifted") && findings[0].contains("`cargo check`"),
        "{findings:?}"
    );
    assert!(findings[1].contains("has no `structure`"), "{findings:?}");
}

#[test]
fn drift_understands_quoted_escapes_and_bare_values() {
    let mut seed = WorkspaceSeed::default();
    seed.commands.push((
        "lint".into(),
        r#"eslint "src/**" --max-warnings 0"#.into(),
        "project.json quality_gate.lint".into(),
    ));
    let body = "[commands]\n\"lint\" = \"eslint \\\"src/**\\\" --max-warnings 0\" # ok\n";
    assert!(seed.drift(body).is_empty(), "{:?}", seed.drift(body));
    assert!(
        seed.drift("").len() == 1,
        "no table means every key is missing"
    );
}

// ── The confirm mode: no dialog in front of a human who is not there ──

/// A project whose `.magistr/workspace.toml` says nothing about the confirm mode gets
/// the token flow declared for it. workspace's default (`elicit`) puts a
/// dialog in front of a human for any write touching more than one file —
/// an ordinary refactor — and under `magistr next-loop` there is nobody
/// there to answer it.
#[test]
fn a_file_that_says_nothing_about_the_confirm_mode_is_given_the_token_flow() {
    let body = "[commands]\n\"test\" = \"cargo test\"\n";
    let out = with_confirm_mode(body).expect("the block is appended");
    assert!(
        out.starts_with(body),
        "nothing the human wrote is disturbed"
    );
    assert!(out.contains("[edit]"));
    assert!(out.contains("confirm_mode = \"token\""));
    assert!(declares_confirm_mode(&out));
}

/// Idempotent: a second `magistr init` must not grow a second `[edit]` block.
#[test]
fn a_file_already_seeded_is_not_seeded_again() {
    let once = with_confirm_mode("[commands]\n").expect("first");
    assert_eq!(with_confirm_mode(&once), None);
}

/// A human who chose `elicit` chose it — in either spelling.
#[test]
fn a_declared_confirm_mode_is_never_overridden() {
    for body in [
        "[edit]\nconfirm_mode = \"elicit\"\n",
        "edit.confirm_mode = \"elicit\"\n",
        "[commands]\n\"t\" = \"x\"\n\n[edit]\nforce_confirm = true\nconfirm_mode = \"elicit\"\n",
    ] {
        assert!(declares_confirm_mode(body), "{body}");
        assert_eq!(with_confirm_mode(body), None, "{body}");
    }
}

/// A `confirm_mode` under some *other* table is not this key.
#[test]
fn a_confirm_mode_under_another_table_is_a_different_key() {
    let body = "[script]\nconfirm_mode = \"elicit\"\n";
    assert!(!declares_confirm_mode(body));
    assert!(with_confirm_mode(body).is_some());
}

/// No `.magistr/workspace.toml` at all means no workspace workspace to configure —
/// writing one here would create the file the commands seed decided not to.
#[test]
fn an_empty_file_is_left_empty() {
    assert_eq!(with_confirm_mode(""), None);
    assert_eq!(with_confirm_mode("\n  \n"), None);
}

/// End to end on a real directory: a project that already declares
/// `[commands]` — the common case, and the one the old early return skipped
/// entirely — still gets the confirm mode, and a second apply writes nothing.
#[test]
fn an_existing_commands_table_still_gets_the_confirm_mode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".magistr")).unwrap();
    let path = tmp.path().join(".magistr/workspace.toml");
    std::fs::write(&path, "[commands]\n\"test\" = \"cargo test\"\n").expect("write");
    let seed = WorkspaceSeed {
        commands: vec![(
            "check".to_owned(),
            "cargo check".to_owned(),
            "gate".to_owned(),
        )],
    };
    assert!(apply(&seed, tmp.path()).expect("apply"));
    let body = std::fs::read_to_string(&path).expect("read");
    assert!(body.contains("confirm_mode = \"token\""), "{body}");
    assert!(
        body.contains("\"test\" = \"cargo test\""),
        "the human's table is untouched: {body}"
    );
    assert!(
        !apply(&seed, tmp.path()).expect("apply"),
        "a second init writes nothing"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("read"), body);
}
