use crate::cli_test_support::parse_cli;

use super::*;

// ── BackendFlag::resolve() ──────────────────────────────────────

#[test]
fn backend_flag_resolve_acp_explicit() {
    let inputs = crate::config::inputs::ProjectInputs::from_lookup("/proj", |_| None);
    let result = BackendFlag::Acp.resolve(&inputs);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), crate::agent::Backend::Acp);
}

// ── parse_backend_str() — tests env var parsing without mutating env ─

#[test]
fn parse_backend_str_acp() {
    assert_eq!(
        BackendFlag::parse_backend_str("acp"),
        Some(BackendFlag::Acp)
    );
}

#[test]
fn parse_backend_str_claude_acp() {
    assert_eq!(
        BackendFlag::parse_backend_str("claude-acp"),
        Some(BackendFlag::Acp)
    );
}

#[test]
fn parse_backend_str_copilot_is_unknown() {
    // Copilot backend was removed — "copilot" should be treated as unrecognized
    assert_eq!(BackendFlag::parse_backend_str("copilot"), None);
}

#[test]
fn parse_backend_str_auto_returns_none() {
    assert_eq!(BackendFlag::parse_backend_str("auto"), None);
}

#[test]
fn parse_backend_str_empty_returns_none() {
    assert_eq!(BackendFlag::parse_backend_str(""), None);
}

#[test]
fn parse_backend_str_unknown_returns_none() {
    assert_eq!(BackendFlag::parse_backend_str("invalid-backend"), None);
}

#[test]
fn parse_backend_str_case_insensitive() {
    assert_eq!(
        BackendFlag::parse_backend_str("ACP"),
        Some(BackendFlag::Acp)
    );
}

#[test]
fn parse_backend_str_whitespace_trimmed() {
    assert_eq!(
        BackendFlag::parse_backend_str("  acp  "),
        Some(BackendFlag::Acp)
    );
}

#[test]
fn parse_backend_str_whitespace_only_returns_none() {
    assert_eq!(BackendFlag::parse_backend_str("   "), None);
}

#[test]
fn deferred_commands_and_flags_are_rejected() {
    for args in [
        vec!["magistr", "--gui"],
        vec!["magistr", "--native"],
        vec!["magistr", "start", "hello", "--native"],
        vec!["magistr", "next", "--native"],
        vec!["magistr", "next-loop", "--native"],
        vec!["magistr", "attach"],
        vec!["magistr", "gui-send", "session"],
        vec!["magistr", "compare", "prompt"],
        vec!["magistr", "directory"],
        vec!["magistr", "init", "--action"],
        vec!["magistr", "keys"],
        vec!["magistr", "start", "hello", "--role", "reviewer"],
        vec!["magistr", "start", "hello", "--roles", "coder,reviewer"],
    ] {
        let error = parse_cli(&args).expect_err("deferred surface is unavailable");
        assert!(
            matches!(
                error.kind(),
                clap::error::ErrorKind::UnknownArgument | clap::error::ErrorKind::InvalidSubcommand
            ),
            "{args:?}: {error}"
        );
    }
    for flag in [
        "--typewriter",
        "--layout",
        "--no-tips",
        "--high-contrast",
        "--keys",
        "--accessible",
        "--sound",
        "--export-summary",
        "--perf",
    ] {
        assert!(parse_cli(["magistr", flag]).is_err(), "{flag}");
    }
}

#[test]
fn start_and_people_replace_run_and_team() {
    let started = parse_cli(["magistr", "start", "hello"]).expect("start parses");
    assert!(matches!(started.command, Some(Command::Start { .. })));
    let error = parse_cli(["magistr", "run", "hello"]).expect_err("run is not a command");
    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::InvalidSubcommand,
        "{error}"
    );
    assert!(matches!(
        parse_cli(["magistr", "people", "show"])
            .expect("people")
            .command,
        Some(Command::People { .. })
    ));
    let error = parse_cli(["magistr", "team", "show"]).expect_err("team is not a command");
    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::InvalidSubcommand,
        "{error}"
    );
}

#[test]
fn loop_flags_use_feature_and_loop() {
    let parsed = parse_cli(["magistr", "next-loop", "--feature", "Billing"]).expect("feature");
    assert!(matches!(
        parsed.command,
        Some(Command::NextLoop { feature: Some(ref name), .. }) if name == "Billing"
    ));
    let resolve = [
        "magistr",
        "resolve-dispatch",
        "--loop",
        "run-1",
        "--command",
        "0/dispatch",
        "--attempt",
        "1",
        "--outcome",
        "failed",
        "--reason",
        "checked",
        "--confirm-stopped",
    ];
    let parsed = parse_cli(resolve).expect("loop");
    assert!(matches!(
        parsed.command,
        Some(Command::ResolveDispatch { ref loop_id, .. }) if loop_id == "run-1"
    ));
    for old in [
        vec!["magistr", "next-loop", "--track", "Billing"],
        vec!["magistr", "resolve-dispatch", "--run", "run-1"],
    ] {
        let error = parse_cli(&old).expect_err("old flag is unknown");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::UnknownArgument,
            "{old:?}: {error}"
        );
    }
}
