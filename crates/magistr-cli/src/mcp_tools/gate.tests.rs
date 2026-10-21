use super::*;

use serde_json::json;

fn config(value: serde_json::Value) -> crate::types::ProjectConfig {
    serde_json::from_value(value).unwrap()
}

fn base_config(archetypes: serde_json::Value) -> crate::types::ProjectConfig {
    config(json!({
        "project": { "name": "test", "description": "", "type": "cli" },
        "stack": {
            "language": "rust", "framework": "none",
            "package_manager": "cargo", "test_framework": "cargo-test",
            "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": {
            "check": "cargo check --workspace",
            "test": "cargo nextest run --workspace",
            "lint": "cargo clippy --workspace"
        },
        "custom_gates": [
            { "name": "structure", "command": "echo structure-ok", "required": true }
        ],
        "format": { "extensions": ["rs"], "command": "cargo fmt" },
        "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "reference_repos": [],
        "state_prefix": "test",
        "archetypes": archetypes
    }))
}

fn archetype(id: &str, root: &str, check: &str, test: &str, lint: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": id,
        "language": "rust",
        "root": root,
        "role": "library",
        "quality_gate": { "check": check, "test": test, "lint": lint },
        "format": { "extensions": ["rs"], "command": "cargo fmt" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "source_patterns": [], "test_patterns": []
    })
}

#[test]
fn resolve_without_archetypes_is_the_top_level() {
    let cfg = base_config(json!([]));
    let specs = resolve_gates(&cfg, "test").unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "test");
    assert_eq!(specs[0].command, "cargo nextest run --workspace");
}

#[test]
fn polyglot_specs_cover_every_stack_once() {
    // The magistr shape: many rust archetypes all naming the project's
    // own workspace-wide command, plus one TS stack at the root. The cargo
    // command must run ONCE (not raced against itself), and bun test must run.
    let cfg = base_config(json!([
        archetype(
            "app",
            ".",
            "cargo check --workspace",
            "cargo nextest run --workspace",
            "cargo clippy --workspace"
        ),
        archetype(
            "core",
            "crates/core",
            "cargo check --workspace",
            "cargo nextest run --workspace",
            "cargo clippy --workspace"
        ),
        archetype(
            "app-typescript",
            ".",
            "npx tsc --noEmit",
            "bun test",
            "bun run lint"
        ),
    ]));

    let specs = resolve_gates(&cfg, "test").unwrap();
    assert_eq!(
        specs
            .iter()
            .map(|s| (s.name.as_str(), s.command.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("test", "cargo nextest run --workspace"),
            ("test:app-typescript", "bun test"),
        ]
    );
}

#[test]
fn a_subdir_archetype_runs_in_its_own_root() {
    let cfg = base_config(json!([
        archetype(
            "app",
            ".",
            "cargo check --workspace",
            "cargo nextest run --workspace",
            "cargo clippy --workspace"
        ),
        archetype("web", "web", "npx tsc --noEmit", "npm test", "npm run lint"),
    ]));

    let specs = resolve_gates(&cfg, "test").unwrap();
    assert_eq!(specs[1].name, "test:web");
    assert_eq!(specs[1].command, "cd web && npm test");
}

#[test]
fn an_archetype_naming_the_project_command_runs_at_the_root() {
    // format-archetype-cwd rule 1 for gates: a subdir archetype carrying the
    // project's own command is the project's gate, run at the project root.
    let cfg = base_config(json!([archetype(
        "core",
        "crates/core",
        "cargo check --workspace",
        "cargo nextest run --workspace",
        "cargo clippy --workspace"
    )]));

    let specs = resolve_gates(&cfg, "test").unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "test");
    assert_eq!(specs[0].command, "cargo nextest run --workspace");
}

#[test]
fn all_covers_every_stack_and_custom_gates_once() {
    let cfg = base_config(json!([
        archetype(
            "app",
            ".",
            "cargo check --workspace",
            "cargo nextest run --workspace",
            "cargo clippy --workspace"
        ),
        archetype(
            "app-typescript",
            ".",
            "npx tsc --noEmit",
            "bun test",
            "bun run lint"
        ),
    ]));

    let specs = resolve_gates(&cfg, "all").unwrap();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "lint",
            "lint:app-typescript",
            "check:app-typescript",
            "test",
            "test:app-typescript",
            "structure",
        ]
    );
}

#[test]
fn an_archetype_with_empty_commands_keeps_its_unconfigured_required_gate() {
    let cfg = base_config(json!([
        archetype(
            "app",
            ".",
            "cargo check --workspace",
            "cargo nextest run --workspace",
            "cargo clippy --workspace"
        ),
        archetype("docs", "docs", "", "", ""),
    ]));

    let specs = resolve_gates(&cfg, "test").unwrap();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].name, "test");
    assert_eq!(specs[0].command, "cargo nextest run --workspace");
    assert_eq!(specs[1].name, "test:docs");
    assert!(specs[1].command.is_empty());
    assert!(
        specs[1].required,
        "missing checks cannot disappear from required coverage"
    );
}

/// `magistr_gate`'s own documentation says "'all' runs standard + required
/// custom gates". It used to extend with *every* custom gate, so a gate
/// registered `required: false` because it is expected to fail (this
/// project's `mock-parity`) was paid for on every `all` and printed a FAIL
/// row that reads like a failed iteration.
#[test]
fn all_runs_only_the_required_custom_gates() {
    let mut cfg = base_config(json!([]));
    cfg.custom_gates.push(crate::types::CustomGate {
        name: "mock-parity".to_owned(),
        command: "echo expected-red; exit 1".to_owned(),
        required: false,
    });

    let specs = resolve_gates(&cfg, "all").unwrap();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["lint", "test", "structure"],
        "a not-required custom gate is not part of `all`"
    );
}

/// …but it is still runnable by name, which is how it is re-blessed.
#[test]
fn a_not_required_custom_gate_is_still_reachable_by_its_own_name() {
    let mut cfg = base_config(json!([]));
    cfg.custom_gates.push(crate::types::CustomGate {
        name: "mock-parity".to_owned(),
        command: "echo expected-red; exit 1".to_owned(),
        required: false,
    });

    let specs = resolve_gates(&cfg, "mock-parity").unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "mock-parity");
    assert!(!specs[0].required);
}

#[test]
fn the_unknown_gate_message_still_names_every_custom_gate() {
    // Excluding a not-required gate from `all` must not hide it from the
    // list a user reads when they misspell one.
    let mut cfg = base_config(json!([]));
    cfg.custom_gates.push(crate::types::CustomGate {
        name: "mock-parity".to_owned(),
        command: "true".to_owned(),
        required: false,
    });
    let message = resolve_gates(&cfg, "nope").unwrap_err();
    assert!(message.contains("mock-parity"), "{message}");
    assert!(message.contains("structure"), "{message}");
}

#[tokio::test]
async fn the_mcp_path_and_the_loop_driver_see_the_same_evidence_for_one_command() {
    // One runner, two callers: the JSON the MCP path answers with is a view
    // over exactly the `GateRun` the loop driver carries, not a second
    // implementation that can drift from it.
    let dir = std::env::current_dir().expect("a working directory");
    let command = "echo 'error[E0308]: mismatched types'; echo '  --> src/x.rs:1:1' 1>&2; exit 7";

    let json = run_gate_command(command, &dir, 10, |_| {}).await.unwrap();
    let run = crate::utils::gate::run_gate(
        "check",
        command,
        true,
        &dir,
        10,
        &tokio_util::sync::CancellationToken::new(),
        |_| {},
    )
    .await;

    assert_eq!(json["passed"], serde_json::json!(run.passed()));
    assert_eq!(json["exit_code"], serde_json::json!(run.exit_code));
    assert_eq!(json["command"], serde_json::json!(run.command));
    assert_eq!(json["output"], serde_json::json!(run.output));
    assert_eq!(json["rows"], serde_json::json!(run.rows));
    assert_eq!(json["verdict"], serde_json::json!("failed"));
    assert_eq!(run.exit_code, 7);
}
