#[path = "cli.test_support.rs"]
mod cli_test_support;
use cli_test_support::parse_cli;

use super::*;

#[test]
fn sanitize_prefix_basic() {
    assert_eq!(utils::slug::sanitize_prefix("My Project"), "my-project");
}

#[test]
fn sanitize_prefix_special_chars() {
    assert_eq!(
        utils::slug::sanitize_prefix("@scope/my-pkg"),
        "scope-my-pkg"
    );
}

#[test]
fn sanitize_prefix_already_clean() {
    assert_eq!(utils::slug::sanitize_prefix("magistr"), "magistr");
}

#[test]
fn full_init_pipeline_detect_to_scaffold() {
    // End-to-end: create a Rust project fixture, detect stack, build vars, generate scaffold.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Minimal Cargo.toml fixture.
    std::fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    let stack = detect::detect_stack(dir).unwrap();
    assert_eq!(stack.language, types::Language::Rust);
    assert_eq!(stack.project_name, "test-crate");

    let preset =
        config::preset::load_preset_with_framework(stack.language, stack.framework).unwrap();
    let default_selections = scaffold::ScaffoldSelections::default();
    let template_vars = commands::misc::build_template_vars(&stack, &preset, &default_selections);
    let written = scaffold::generate(
        dir,
        &template_vars,
        &std::collections::HashMap::new(),
        &scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // At least 20 scaffold files (templates + mcp.json + gitignore + plugins).
    // Exact count changes as templates are added — assert a minimum, not exact.
    assert!(
        written.len() >= 20,
        "Expected at least 20 scaffold files, got {}",
        written.len()
    );

    // Verify project.json has real values from detection.
    let pj = std::fs::read_to_string(dir.join(".magistr/project.json")).unwrap();
    assert!(pj.contains("test-crate"));
    assert!(pj.contains("\"language\": \"rust\""));
    assert!(pj.contains("cargo check"));

    // CLAUDE.md references the project.
    let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("test-crate"));
    assert!(claude.contains("cargo"));
}

#[test]
fn completions_generates_bash() {
    let mut cmd = <cli::Cli as clap::CommandFactory>::command();
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Bash, &mut cmd, "magistr", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("magistr"));
    assert!(!output.is_empty());
}

#[test]
fn completions_generates_zsh() {
    let mut cmd = <cli::Cli as clap::CommandFactory>::command();
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Zsh, &mut cmd, "magistr", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("magistr"));
}

#[test]
fn completions_generates_fish() {
    let mut cmd = <cli::Cli as clap::CommandFactory>::command();
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Fish, &mut cmd, "magistr", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("magistr"));
}

#[test]
fn man_page_generates() {
    let cmd = <cli::Cli as clap::CommandFactory>::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf).unwrap();
    assert!(!buf.is_empty());
    // Man pages start with .TH header
    let content = String::from_utf8_lossy(&buf);
    assert!(content.contains(".TH"));
    assert!(content.contains("magistr"));
}

#[test]
fn man_page_writes_to_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    commands::misc::cmd_man_pages(tmp.path()).unwrap();
    let man_path = tmp.path().join("magistr.1");
    assert!(man_path.exists());
    let content = std::fs::read_to_string(man_path).unwrap();
    assert!(content.contains("magistr"));
}

#[test]
fn json_detect_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "test-proj"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    let stack = detect::detect_stack(tmp.path()).unwrap();
    let envelope = output::success(&stack);
    let json = serde_json::to_string_pretty(&envelope).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["language"], "rust");
    assert_eq!(parsed["data"]["project_name"], "test-proj");
}

#[test]
fn cli_parses_json_flag() {
    let cli = parse_cli(["magistr", "--json", "detect"]).unwrap();
    assert!(cli.json);
}

#[test]
fn cli_parses_completions() {
    let cli = parse_cli(["magistr", "completions", "bash"]).unwrap();
    matches!(cli.command, Some(cli::Command::Completions { .. }));
}

#[test]
fn cli_parses_man_pages() {
    let cli = parse_cli(["magistr", "man-pages", "/tmp"]).unwrap();
    matches!(cli.command, Some(cli::Command::ManPages { .. }));
}

#[test]
fn cli_accepts_explicit_codex_scaffold_selection() {
    for mode in ["--yes", "--dry-run"] {
        let parsed = parse_cli(["magistr", "init", mode, "--agent", "codex"]);
        assert!(
            parsed.is_ok(),
            "{mode} should accept a Codex target: {parsed:?}"
        );
    }
}

#[test]
fn cli_scaffold_targets_are_repeatable_and_validated() {
    let parsed = parse_cli([
        "magistr",
        "init",
        "--agent",
        "codex,cursor",
        "--agent",
        "copilot",
    ])
    .unwrap();
    match parsed.command.unwrap() {
        cli::Command::Init { agents, .. } => assert_eq!(agents, ["codex", "cursor", "copilot"]),
        other => panic!("expected Init, got {other:?}"),
    }
    assert!(parse_cli(["magistr", "init", "--agent", "codxe"]).is_err());
    for mode in ["--interactive", "--ci", "--check"] {
        assert!(parse_cli(["magistr", "init", mode, "--agent", "codex"]).is_err());
    }
}

#[test]
fn cli_codex_launch_forwards_literal_arguments() {
    let parsed = parse_cli([
        "magistr",
        "codex-launch",
        "--project-root",
        "/project with spaces",
        "--",
        "--model",
        "user-model",
        "literal $(touch nope); value",
        "",
    ])
    .unwrap();
    match parsed.command.unwrap() {
        cli::Command::CodexLaunch {
            project_root,
            diagnose,
            args,
        } => {
            assert_eq!(
                project_root,
                std::path::PathBuf::from("/project with spaces")
            );
            assert!(!diagnose);
            assert_eq!(
                args,
                ["--model", "user-model", "literal $(touch nope); value", ""]
                    .map(std::ffi::OsString::from)
            );
        }
        other => panic!("expected CodexLaunch, got {other:?}"),
    }
}

#[test]
fn cli_codex_removal_addresses_the_selected_project() {
    let parsed = parse_cli(["magistr", "codex-remove", "/project with spaces"]).unwrap();
    match parsed.command.unwrap() {
        cli::Command::CodexRemove { dir, dry_run } => {
            assert_eq!(dir, std::path::PathBuf::from("/project with spaces"));
            assert!(!dry_run, "a removal applies unless a preview is asked for");
        }
        other => panic!("expected CodexRemove, got {other:?}"),
    }
}

#[test]
fn cli_codex_removal_dry_run_is_a_preview() {
    for flag in ["--dry-run", "-n"] {
        let parsed = parse_cli(["magistr", "codex-remove", flag]).unwrap();
        match parsed.command.unwrap() {
            cli::Command::CodexRemove { dir, dry_run } => {
                assert_eq!(dir, std::path::PathBuf::from("."));
                assert!(dry_run, "{flag}");
            }
            other => panic!("expected CodexRemove, got {other:?}"),
        }
    }
}

#[test]
fn cli_parses_guided_init_default() {
    let cli = parse_cli(["magistr", "init"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { yes, force, .. } => {
            assert!(!yes);
            assert!(!force);
        }
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_parses_init_force() {
    let cli = parse_cli(["magistr", "init", "--force"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { force, .. } => assert!(force),
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_parses_init_force_short() {
    let cli = parse_cli(["magistr", "init", "-f"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { force, .. } => assert!(force),
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_parses_init_dry_run() {
    let cli = parse_cli(["magistr", "init", "--dry-run"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_parses_init_dry_run_short() {
    let cli = parse_cli(["magistr", "init", "-n"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_dry_run_conflicts_with_force() {
    let result = parse_cli(["magistr", "init", "--dry-run", "--force"]);
    assert!(result.is_err(), "--dry-run and --force should conflict");
}

#[test]
fn cli_parses_init_ci_flag() {
    let cli = parse_cli(["magistr", "init", "--ci"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { ci, .. } => assert!(ci),
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_init_ci_defaults_false() {
    let cli = parse_cli(["magistr", "init"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Init { ci, .. } => assert!(!ci),
        _ => panic!("expected Init command"),
    }
}

#[test]
fn cli_init_ci_conflicts_with_interactive() {
    let result = parse_cli(["magistr", "init", "--ci", "-i"]);
    assert!(result.is_err(), "--ci and --interactive should conflict");
}

#[test]
fn cli_init_ci_conflicts_with_dry_run() {
    let result = parse_cli(["magistr", "init", "--ci", "--dry-run"]);
    assert!(result.is_err(), "--ci and --dry-run should conflict");
}

#[test]
fn remove_scaffold_deletes_generated_files_and_preserves_unowned_scripts() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    generate_removal_fixture(dir);

    std::fs::write(dir.join("run-autonomous.sh"), "#!/bin/bash").unwrap();
    std::fs::write(dir.join("format-stream.sh"), "#!/bin/bash").unwrap();
    std::fs::write(
        dir.join(".mcp.json"),
        r#"{"mcpServers":{"magistr":{"command":"magistr"},"magistr-scope":{"command":"magistr-scope"}}}"#,
    ).unwrap();
    std::fs::write(dir.join("CLAUDE.md"), "# My project").unwrap();
    std::fs::write(dir.join("ROADMAP.md"), "# Roadmap").unwrap();

    scaffold::remove_scaffold(dir).unwrap();

    assert!(!dir.join(".claude").exists());
    assert!(!dir.join(".magistr").exists());
    assert!(!dir.join(".mcp.json").exists());
    assert!(dir.join("run-autonomous.sh").exists());
    assert!(dir.join("format-stream.sh").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap(),
        "# My project"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("ROADMAP.md")).unwrap(),
        "# Roadmap"
    );
}

fn generate_removal_fixture(dir: &std::path::Path) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"cleanup-fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let stack = detect::detect_stack(dir).unwrap();
    let preset =
        config::preset::load_preset_with_framework(stack.language, stack.framework).unwrap();
    let vars = commands::misc::build_template_vars(
        &stack,
        &preset,
        &scaffold::ScaffoldSelections::default(),
    );
    scaffold::generate(
        dir,
        &vars,
        &std::collections::HashMap::new(),
        &scaffold::SetupSeeds::default(),
    )
    .unwrap();
}

#[test]
fn ensure_no_existing_scaffold_ok_without_force() {
    // Idempotent: existing .claude/ without --force is now fine.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir(dir.join(".claude")).unwrap();

    let result = commands::misc::ensure_no_existing_scaffold(dir, false);
    assert!(result.is_ok());
    // .claude/ should still exist (not removed).
    assert!(dir.join(".claude").exists());
}

#[test]
fn ensure_no_existing_scaffold_preserves_with_force() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    generate_removal_fixture(dir);
    let manifest = std::fs::read(dir.join(".magistr/scaffold-manifest.json")).unwrap();

    commands::misc::ensure_no_existing_scaffold(dir, true).unwrap();
    assert!(dir.join(".claude/skills/next/SKILL.md").is_file());
    assert_eq!(
        std::fs::read(dir.join(".magistr/scaffold-manifest.json")).unwrap(),
        manifest
    );
}

#[test]
fn ensure_no_existing_scaffold_ok_when_no_claude_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    commands::misc::ensure_no_existing_scaffold(tmp.path(), false).unwrap();
    commands::misc::ensure_no_existing_scaffold(tmp.path(), true).unwrap();
}

#[test]
fn cli_parses_export() {
    let cli = parse_cli(["magistr", "export"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Export { .. })));
}

#[test]
fn cli_parses_export_with_output() {
    let cli = parse_cli(["magistr", "export", "-o", "my-preset.toml"]).unwrap();
    if let cli::Command::Export { output, .. } = cli.command.unwrap() {
        assert_eq!(output.unwrap().to_str().unwrap(), "my-preset.toml");
    } else {
        panic!("expected Export command");
    }
}

#[test]
fn cli_parses_import() {
    let cli = parse_cli(["magistr", "import", "team.toml"]).unwrap();
    if let cli::Command::Import { source, .. } = cli.command.unwrap() {
        assert_eq!(source, "team.toml");
    } else {
        panic!("expected Import command");
    }
}

#[test]
fn cli_parses_init_from() {
    let cli = parse_cli(["magistr", "init", "--from", "preset.toml"]).unwrap();
    if let cli::Command::Init { from, .. } = cli.command.unwrap() {
        assert_eq!(from.unwrap(), "preset.toml");
    } else {
        panic!("expected Init command");
    }
}

#[test]
fn cli_init_from_conflicts_with_smart() {
    let result = parse_cli(["magistr", "init", "--from", "p.toml", "--smart"]);
    assert!(result.is_err(), "--from and --smart should conflict");
}

#[test]
fn parse_language_or_unknown_valid() {
    assert_eq!(
        commands::config::parse_language_or_unknown("rust"),
        types::Language::Rust
    );
    assert_eq!(
        commands::config::parse_language_or_unknown("typescript"),
        types::Language::TypeScript
    );
    assert_eq!(
        commands::config::parse_language_or_unknown("python"),
        types::Language::Python
    );
}

#[test]
fn parse_language_or_unknown_fallback() {
    assert_eq!(
        commands::config::parse_language_or_unknown("brainfuck"),
        types::Language::Unknown
    );
    assert_eq!(
        commands::config::parse_language_or_unknown(""),
        types::Language::Unknown
    );
}

#[test]
fn cli_parses_costs() {
    let cli = parse_cli(["magistr", "costs"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Costs { .. })));
}

#[test]
fn cli_parses_costs_with_project_filter() {
    let cli = parse_cli([
        "magistr",
        "costs",
        "--project",
        "my-proj",
        "--since-days",
        "7",
    ])
    .unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.project.as_deref(), Some("my-proj"));
        assert_eq!(query.since_days, Some(7));
        assert_eq!(query.limit, 20); // default
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_parses_budget_flags() {
    let cli = parse_cli([
        "magistr",
        "--budget",
        "5.00",
        "--budget-warn",
        "3.00",
        "detect",
        ".",
    ])
    .unwrap();
    assert_eq!(cli.budget, Some(5.0));
    assert_eq!(cli.budget_warn, Some(3.0));
}

#[test]
fn cli_budget_flags_default_to_none() {
    let cli = parse_cli(["magistr", "detect", "."]).unwrap();
    assert_eq!(cli.budget, None);
    assert_eq!(cli.budget_warn, None);
}

#[test]
fn cli_parses_model_flag() {
    let cli = parse_cli(["magistr", "run", "--model", "sonnet", "fix the bug"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Run { agent, .. } => {
            let model = agent.model;
            assert_eq!(model.as_deref(), Some("sonnet"));
        }
        _ => panic!("expected Run command"),
    }
}

#[test]
fn cli_parses_model_flag_short() {
    let cli = parse_cli(["magistr", "run", "-m", "opus", "refactor this"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Run { agent, .. } => {
            let model = agent.model;
            assert_eq!(model.as_deref(), Some("opus"));
        }
        _ => panic!("expected Run command"),
    }
}

#[test]
fn cli_run_model_defaults_to_none() {
    let cli = parse_cli(["magistr", "run", "hello"]).unwrap();
    match cli.command.unwrap() {
        cli::Command::Run { agent, .. } => {
            let model = agent.model;
            assert!(model.is_none());
        }
        _ => panic!("expected Run command"),
    }
}

#[test]
fn budget_config_merges_cli_with_user_config() {
    use magistr::config::user_config::BudgetDefaults;
    use magistr::cost::BudgetConfig;

    // CLI overrides config defaults
    let config_defaults = BudgetDefaults {
        limit: Some(10.0),
        warn_threshold: Some(7.0),
    };
    let cli_limit: Option<f64> = Some(5.0);
    let cli_warn: Option<f64> = None;

    let merged = BudgetConfig::from_cli_and_defaults(
        cli_limit,
        cli_warn,
        config_defaults.limit,
        config_defaults.warn_threshold,
    );
    assert_eq!(merged.limit, Some(5.0)); // CLI wins
    assert_eq!(merged.warn_threshold, Some(7.0)); // Config fallback
}

#[test]
fn budget_config_falls_back_to_user_config() {
    use magistr::config::user_config::BudgetDefaults;
    use magistr::cost::BudgetConfig;

    let config_defaults = BudgetDefaults {
        limit: Some(20.0),
        warn_threshold: Some(15.0),
    };

    let merged = BudgetConfig::from_cli_and_defaults(
        None,
        None,
        config_defaults.limit,
        config_defaults.warn_threshold,
    );
    assert_eq!(merged.limit, Some(20.0));
    assert_eq!(merged.warn_threshold, Some(15.0));
}

#[test]
fn cli_parses_costs_group_by_user() {
    let cli = parse_cli(["magistr", "costs", "--group-by", "user"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.group_by, Some(cli::CostGroupBy::User));
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_parses_costs_chargeback_with_a_period() {
    let cli = parse_cli([
        "magistr",
        "costs",
        "--chargeback",
        "--chargeback-period",
        "weekly",
    ])
    .unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert!(query.chargeback);
        assert_eq!(
            query.chargeback_period,
            Some(cli::CostSummaryPeriod::Weekly)
        );
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_costs_policy_defaults_to_the_current_directory() {
    let cli = parse_cli(["magistr", "costs", "--policy"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.policy.as_deref(), Some(std::path::Path::new(".")));
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_costs_group_by_and_summary_are_mutually_exclusive() {
    assert!(
        parse_cli([
            "magistr",
            "costs",
            "--group-by",
            "user",
            "--summary",
            "daily"
        ])
        .is_err()
    );
}

#[test]
fn cli_costs_chargeback_period_requires_chargeback() {
    assert!(parse_cli(["magistr", "costs", "--chargeback-period", "daily"]).is_err());
}

#[test]
fn a_roster_cap_overrides_the_users_own_budget_defaults() {
    use magistr::config::user_config::BudgetDefaults;
    use magistr::cost::governed_budget;
    use magistr::team::TeamConfig;
    use magistr::team::directory::DirectoryConfig;
    use magistr_engine::cost::{BudgetConfig, BudgetPeriod, BudgetRule, BudgetScope};

    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");

    // A team that caps every session at two dollars.
    let mut roster = TeamConfig {
        version: 1,
        default_role: magistr::team::TeamRole::Developer,
        identity_provider: None,
        budgets: vec![BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(2.0)],
        policy: Default::default(),
        members: Vec::new(),
        directory: DirectoryConfig::default(),
    };
    roster.budgets[0].warn_threshold = Some(1.5);
    roster.save(dir.path()).expect("save roster");

    // A developer who set themselves a generous default and passed an even
    // more generous flag.
    let defaults = BudgetDefaults {
        limit: Some(100.0),
        warn_threshold: Some(90.0),
    };
    let local = BudgetConfig {
        limit: Some(999.0).or(defaults.limit),
        warn_threshold: defaults.warn_threshold,
        max_iterations: None,
        max_duration_secs: None,
    };

    let resolved = governed_budget(dir.path(), &ledger, &local, chrono::Utc::now());
    assert_eq!(resolved.effective.limit, Some(2.0));
    assert_eq!(resolved.effective.warn_threshold, Some(1.5));
    assert_eq!(resolved.binding, Some(BudgetScope::Session));
}

#[test]
fn cli_parses_costs_format_csv() {
    let cli = parse_cli(["magistr", "costs", "--format", "csv"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.format, cli::CostFormat::Csv);
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_parses_costs_format_json() {
    let cli = parse_cli(["magistr", "costs", "--format", "json"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.format, cli::CostFormat::Json);
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_costs_format_defaults_to_table() {
    let cli = parse_cli(["magistr", "costs"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.format, cli::CostFormat::Table);
        assert!(query.summary.is_none());
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_parses_costs_summary_daily() {
    let cli = parse_cli(["magistr", "costs", "--summary", "daily"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.summary, Some(cli::CostSummaryPeriod::Daily));
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_parses_costs_summary_monthly() {
    let cli = parse_cli(["magistr", "costs", "--summary", "monthly"]).unwrap();
    if let cli::Command::Costs { query } = cli.command.unwrap() {
        assert_eq!(query.summary, Some(cli::CostSummaryPeriod::Monthly));
    } else {
        panic!("expected Costs command");
    }
}

#[test]
fn cli_parses_plans() {
    let cli = parse_cli(["magistr", "plans"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Plans { .. })));
}

#[test]
fn cli_parses_plans_with_status_filter() {
    let cli = parse_cli(["magistr", "plans", "--status", "completed"]).unwrap();
    if let cli::Command::Plans { status, .. } = cli.command.unwrap() {
        assert_eq!(status, Some(cli::PlanStatusFilter::Completed));
    } else {
        panic!("expected Plans command");
    }
}

#[test]
fn cli_parses_plans_with_limit() {
    let cli = parse_cli(["magistr", "plans", "-n", "5"]).unwrap();
    if let cli::Command::Plans { limit, .. } = cli.command.unwrap() {
        assert_eq!(limit, 5);
    } else {
        panic!("expected Plans command");
    }
}

#[test]
fn cli_parses_next_continue() {
    let cli = parse_cli(["magistr", "next", "--continue"]).unwrap();
    if let cli::Command::Next {
        r#continue: cont, ..
    } = cli.command.unwrap()
    {
        assert!(cont);
    } else {
        panic!("expected Next command");
    }
}

#[test]
fn cli_parses_next_continue_short() {
    let cli = parse_cli(["magistr", "next", "-c"]).unwrap();
    if let cli::Command::Next {
        r#continue: cont, ..
    } = cli.command.unwrap()
    {
        assert!(cont);
    } else {
        panic!("expected Next command");
    }
}

#[test]
fn cli_next_defaults_no_continue() {
    let cli = parse_cli(["magistr", "next"]).unwrap();
    if let cli::Command::Next {
        r#continue: cont, ..
    } = cli.command.unwrap()
    {
        assert!(!cont);
    } else {
        panic!("expected Next command");
    }
}

#[test]
fn cli_parses_sessions() {
    let cli = parse_cli(["magistr", "sessions"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Sessions { .. })));
}

#[test]
fn cli_parses_sessions_with_project_filter() {
    let cli = parse_cli(["magistr", "sessions", "-p", "magistr"]).unwrap();
    if let cli::Command::Sessions { project, .. } = cli.command.unwrap() {
        assert_eq!(project.as_deref(), Some("magistr"));
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_sessions_with_limit() {
    let cli = parse_cli(["magistr", "sessions", "-n", "50"]).unwrap();
    if let cli::Command::Sessions { limit, .. } = cli.command.unwrap() {
        assert_eq!(limit, 50);
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_sessions_json_format() {
    let cli = parse_cli(["magistr", "sessions", "--format", "json"]).unwrap();
    if let cli::Command::Sessions { format, .. } = cli.command.unwrap() {
        assert_eq!(format, cli::SessionFormat::Json);
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_sessions_format_defaults_to_table() {
    let cli = parse_cli(["magistr", "sessions"]).unwrap();
    if let cli::Command::Sessions { format, .. } = cli.command.unwrap() {
        assert_eq!(format, cli::SessionFormat::Table);
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_sessions_since_days() {
    let cli = parse_cli(["magistr", "sessions", "--since-days", "7"]).unwrap();
    if let cli::Command::Sessions { since_days, .. } = cli.command.unwrap() {
        assert_eq!(since_days, Some(7));
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_sessions_owner_filter() {
    let cli = parse_cli(["magistr", "sessions", "--owner", "alice"]).unwrap();
    if let cli::Command::Sessions { owner, .. } = cli.command.unwrap() {
        assert_eq!(owner.as_deref(), Some("alice"));
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_sessions_handoff_pair() {
    let cli = parse_cli([
        "magistr",
        "sessions",
        "--handoff",
        "sess-1",
        "bob@example.dev",
    ])
    .unwrap();
    if let cli::Command::Sessions { handoff, .. } = cli.command.unwrap() {
        assert_eq!(
            handoff,
            Some(vec!["sess-1".to_string(), "bob@example.dev".to_string()])
        );
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_sessions_handoff_requires_two_values() {
    let result = parse_cli(["magistr", "sessions", "--handoff", "sess-1"]);
    assert!(result.is_err());
}

#[test]
fn cli_parses_sessions_purge() {
    let cli = parse_cli(["magistr", "sessions", "--purge", "90"]).unwrap();
    if let cli::Command::Sessions { purge, .. } = cli.command.unwrap() {
        assert_eq!(purge, Some(90));
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_sessions_tag() {
    let cli = parse_cli(["magistr", "sessions", "--tag", "sess-123", "auth-refactor"]).unwrap();
    if let cli::Command::Sessions { tag, .. } = cli.command.unwrap() {
        let tag = tag.unwrap();
        assert_eq!(tag, vec!["sess-123", "auth-refactor"]);
    } else {
        panic!("expected Sessions command");
    }
}

#[test]
fn cli_parses_trust_flag() {
    let cli = parse_cli(["magistr", "--trust", "trusted", "detect", "."]).unwrap();
    assert_eq!(cli.trust.as_deref(), Some("trusted"));
}

#[test]
fn cli_trust_defaults_to_none() {
    let cli = parse_cli(["magistr", "detect", "."]).unwrap();
    assert!(cli.trust.is_none());
}

#[test]
fn cli_parses_permissions() {
    let cli = parse_cli(["magistr", "permissions"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(cli::Command::Permissions { .. })
    ));
}

#[test]
fn cli_parses_permissions_with_limit() {
    let cli = parse_cli(["magistr", "permissions", "-n", "100"]).unwrap();
    if let cli::Command::Permissions { limit, .. } = cli.command.unwrap() {
        assert_eq!(limit, 100);
    } else {
        panic!("expected Permissions command");
    }
}

#[test]
fn cli_parses_permissions_with_project() {
    let cli = parse_cli(["magistr", "permissions", "-p", "my-project"]).unwrap();
    if let cli::Command::Permissions { project, .. } = cli.command.unwrap() {
        assert_eq!(project.as_deref(), Some("my-project"));
    } else {
        panic!("expected Permissions command");
    }
}

#[test]
fn cli_parses_permissions_with_actor() {
    let cli = parse_cli(["magistr", "permissions", "-a", "dev@example.com"]).unwrap();
    if let cli::Command::Permissions { actor, .. } = cli.command.unwrap() {
        assert_eq!(actor.as_deref(), Some("dev@example.com"));
    } else {
        panic!("expected Permissions command");
    }
}

#[test]
fn cli_parses_permissions_export_formats() {
    for (arg, expected) in [
        ("json", cli::ExportFormat::Json),
        ("csv", cli::ExportFormat::Csv),
    ] {
        let cli = parse_cli(["magistr", "permissions", "--export", arg]).unwrap();
        if let cli::Command::Permissions { export, .. } = cli.command.unwrap() {
            assert_eq!(export, Some(expected));
        } else {
            panic!("expected Permissions command");
        }
    }
}

#[test]
fn cli_rejects_unknown_export_format() {
    assert!(parse_cli(["magistr", "permissions", "--export", "xml"]).is_err());
}

#[test]
fn cli_parses_models() {
    let cli = parse_cli(["magistr", "models"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Models)));
}

#[test]
fn cli_parses_learn() {
    let cli = parse_cli(["magistr", "learn"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Learn { .. })));
}

#[test]
fn cli_parses_learn_with_dir() {
    let cli = parse_cli(["magistr", "learn", "/tmp/project"]).unwrap();
    if let cli::Command::Learn { dir } = cli.command.unwrap() {
        assert_eq!(dir, std::path::PathBuf::from("/tmp/project"));
    } else {
        panic!("expected Learn command");
    }
}

#[test]
fn cli_parses_memory() {
    let cli = parse_cli(["magistr", "memory"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Memory { .. })));
}

#[test]
fn cli_parses_memory_show() {
    let cli = parse_cli(["magistr", "memory", "show"]).unwrap();
    if let cli::Command::Memory { action, .. } = cli.command.unwrap() {
        assert!(matches!(action, Some(cli::MemoryAction::Show)));
    } else {
        panic!("expected Memory command");
    }
}

#[test]
fn cli_parses_memory_add() {
    let cli = parse_cli([
        "magistr",
        "memory",
        "add",
        "some observation",
        "-c",
        "patterns",
    ])
    .unwrap();
    if let cli::Command::Memory {
        action: Some(cli::MemoryAction::Add { text, category }),
        ..
    } = cli.command.unwrap()
    {
        assert_eq!(text, "some observation");
        assert_eq!(category, "patterns");
    } else {
        panic!("expected Memory Add command");
    }
}

#[test]
fn cli_parses_memory_reset() {
    let cli = parse_cli(["magistr", "memory", "reset"]).unwrap();
    if let cli::Command::Memory { action, .. } = cli.command.unwrap() {
        assert!(matches!(action, Some(cli::MemoryAction::Reset)));
    } else {
        panic!("expected Memory command");
    }
}

#[test]
fn cli_parses_validate_ci_flag() {
    let cli = parse_cli(["magistr", "validate", "--ci"]).unwrap();
    if let cli::Command::Validate { ci, .. } = cli.command.unwrap() {
        assert!(ci);
    } else {
        panic!("expected Validate command");
    }
}

#[test]
fn cli_validate_ci_flag_defaults_false() {
    let cli = parse_cli(["magistr", "validate"]).unwrap();
    if let cli::Command::Validate { ci, .. } = cli.command.unwrap() {
        assert!(!ci);
    } else {
        panic!("expected Validate command");
    }
}

#[test]
fn cli_parses_completions_install_flag() {
    let cli = parse_cli(["magistr", "completions", "bash", "--install"]).unwrap();
    if let cli::Command::Completions { shell, install } = cli.command.unwrap() {
        assert_eq!(shell, clap_complete::Shell::Bash);
        assert!(install);
    } else {
        panic!("expected Completions command");
    }
}

#[test]
fn cli_completions_install_defaults_false() {
    let cli = parse_cli(["magistr", "completions", "zsh"]).unwrap();
    if let cli::Command::Completions { install, .. } = cli.command.unwrap() {
        assert!(!install);
    } else {
        panic!("expected Completions command");
    }
}

#[test]
fn completions_install_path_powershell_returns_none() {
    let path = commands::misc::completions_install_path(clap_complete::Shell::PowerShell);
    assert!(path.is_none());
}

#[test]
fn cli_parses_plugins_list() {
    let cli = parse_cli(["magistr", "plugins"]).unwrap();
    assert!(matches!(cli.command, Some(cli::Command::Plugins { .. })));
}

#[test]
fn cli_parses_plugins_list_explicit() {
    let cli = parse_cli(["magistr", "plugins", "list"]).unwrap();
    if let cli::Command::Plugins { action } = cli.command.unwrap() {
        assert!(matches!(action, Some(cli::PluginAction::List)));
    } else {
        panic!("expected Plugins command");
    }
}

#[test]
fn cli_parses_plugins_add() {
    let cli = parse_cli(["magistr", "plugins", "add", "/tmp/my-plugin"]).unwrap();
    if let cli::Command::Plugins {
        action: Some(cli::PluginAction::Add { path }),
    } = cli.command.unwrap()
    {
        assert_eq!(path, std::path::PathBuf::from("/tmp/my-plugin"));
    } else {
        panic!("expected Plugins Add command");
    }
}

#[test]
fn cli_parses_plugins_remove() {
    let cli = parse_cli(["magistr", "plugins", "remove", "my-plugin"]).unwrap();
    if let cli::Command::Plugins {
        action: Some(cli::PluginAction::Remove { name }),
    } = cli.command.unwrap()
    {
        assert_eq!(name, "my-plugin");
    } else {
        panic!("expected Plugins Remove command");
    }
}

#[test]
fn cli_parses_plugins_validate() {
    let cli = parse_cli(["magistr", "plugins", "validate", "/tmp/my-plugin"]).unwrap();
    if let cli::Command::Plugins {
        action: Some(cli::PluginAction::Validate { path }),
    } = cli.command.unwrap()
    {
        assert_eq!(path, std::path::PathBuf::from("/tmp/my-plugin"));
    } else {
        panic!("expected Plugins Validate command");
    }
}

#[test]
fn cli_no_subcommand_returns_none() {
    let cli = parse_cli(["magistr"]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn command_dispatch_routes_to_correct_module() {
    // Verify that the dispatch table in main() covers all Command variants.
    // We can't actually run async commands here, but we verify the CLI parses
    // and the command modules are accessible.
    assert!(commands::misc::completions_install_path(clap_complete::Shell::PowerShell).is_none());

    // Verify resolve_backend works for ACP
    let inputs = magistr::config::inputs::ProjectInputs::from_lookup("/proj", |_| None);
    let backend = commands::resolve_backend(cli::BackendFlag::Acp, &inputs).unwrap();
    assert!(matches!(backend, agent::Backend::Acp));
}

// ── guard (skill-hook-claims) ──────────────────────────────────────

#[test]
fn cli_parses_guard_stop_and_commit() {
    for (word, expect) in [
        ("stop", cli::GuardEvent::Stop),
        ("commit", cli::GuardEvent::Commit),
    ] {
        let parsed = parse_cli(["magistr", "guard", word, "/tmp/p"]).unwrap();
        match parsed.command.unwrap() {
            cli::Command::Guard { event, dir } => {
                assert_eq!(event, expect);
                assert_eq!(dir, std::path::PathBuf::from("/tmp/p"));
            }
            other => panic!("expected Guard, got {other:?}"),
        }
    }
    assert!(parse_cli(["magistr", "guard", "push"]).is_err());
}

// ── roadmap subcommands (ts4a) ──────────────────────────────────────

#[test]
fn cli_parses_roadmap_next_status_and_export() {
    for (args, expect) in [
        (["magistr", "roadmap", "next"], "Next"),
        (["magistr", "roadmap", "status"], "Status"),
        (["magistr", "roadmap", "export"], "Export"),
    ] {
        let parsed = parse_cli(args).unwrap();
        let Some(cli::Command::Roadmap { action }) = parsed.command else {
            panic!("expected a roadmap command for {args:?}");
        };
        let name = match action {
            Some(cli::RoadmapAction::Next { .. }) => "Next",
            Some(cli::RoadmapAction::Status { .. }) => "Status",
            Some(cli::RoadmapAction::Export { .. }) => "Export",
            other => panic!("unexpected action {other:?}"),
        };
        assert_eq!(name, expect);
    }
}

#[test]
fn cli_roadmap_sync_is_an_alias_of_export_and_reset_is_gone() {
    let parsed = parse_cli(["magistr", "roadmap", "sync"]).unwrap();
    assert!(matches!(
        parsed.command,
        Some(cli::Command::Roadmap {
            action: Some(cli::RoadmapAction::Export { .. })
        })
    ));
    assert!(
        parse_cli(["magistr", "roadmap", "reset"]).is_err(),
        "`roadmap reset` was removed with the YAML-only semantics"
    );
}

/// The maintained system-contract document, read from the repository.
fn system_contracts_doc() -> (std::path::PathBuf, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/architecture/system-contracts.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()));
    (path, text)
}

#[test]
fn every_shipped_command_has_a_row_in_the_system_contract_entry_matrix() {
    let (_, doc) = system_contracts_doc();
    let missing: Vec<String> = <cli::Cli as clap::CommandFactory>::command()
        .get_subcommands()
        .map(|sub| sub.get_name().to_owned())
        .filter(|name| name != "help")
        .filter(|name| !doc.contains(&format!("`magistr {name}")))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/architecture/system-contracts.md has no entry-matrix row for: {missing:?}"
    );
}

#[test]
fn every_relative_link_in_the_system_contract_document_resolves() {
    let (path, doc) = system_contracts_doc();
    let base = path.parent().expect("the document has a directory");
    let broken: Vec<&str> = doc
        .split("](")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .filter(|target| !target.starts_with("http") && !target.starts_with('#'))
        .filter(|target| {
            let file = target.split('#').next().unwrap_or(target);
            !base.join(file).exists()
        })
        .collect();
    assert!(broken.is_empty(), "unresolved links: {broken:?}");
}
