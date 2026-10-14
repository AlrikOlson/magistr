use super::*;

#[test]
fn io_not_found_has_suggestion() {
    let err = MagistrError::io(
        "/tmp/nonexistent",
        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    );
    let suggestion = err.suggestion();
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("path exists"));
}

#[test]
fn io_permission_denied_has_suggestion() {
    let err = MagistrError::io(
        "/tmp/secret",
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    );
    let suggestion = err.suggestion();
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("permissions"));
}

#[test]
fn io_other_kind_has_no_suggestion() {
    let err = MagistrError::io(
        "/tmp/broken",
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe"),
    );
    assert!(err.suggestion().is_none());
}

#[test]
fn detection_has_suggestion() {
    let err = MagistrError::Detection("no manifest found".into());
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("manifest file"));
}

#[test]
fn preset_not_found_has_suggestion() {
    let err = MagistrError::PresetNotFound("brainfuck".into());
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("magistr detect"));
}

#[test]
fn config_has_suggestion() {
    let err = MagistrError::Config("invalid JSON".into());
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("magistr init"));
}

#[test]
fn dir_not_found_has_suggestion() {
    let err = MagistrError::DirNotFound("/nonexistent".into());
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("directory path"));
}

#[test]
fn template_has_no_suggestion() {
    let err = MagistrError::Template("missing placeholder".into());
    assert!(err.suggestion().is_none());
}

#[test]
fn budget_has_suggestion() {
    let err = MagistrError::Budget("$5.00 limit exceeded".into());
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("--budget"));
}

#[test]
fn other_has_no_suggestion() {
    let err = MagistrError::Other("something went wrong".into());
    assert!(err.suggestion().is_none());
}

#[test]
fn cancelled_has_no_suggestion() {
    let err = MagistrError::Cancelled("user aborted".into());
    assert!(err.suggestion().is_none());
}

#[test]
fn plugin_has_suggestion() {
    let err = MagistrError::Plugin("manifest missing".into());
    assert!(err.suggestion().unwrap().contains("plugins validate"));
}

#[test]
fn workflow_has_suggestion() {
    let err = MagistrError::Workflow("plan file missing".into());
    assert!(err.suggestion().unwrap().contains("magistr init"));
}

#[test]
fn serialize_has_suggestion() {
    let err = MagistrError::Serialize("failed to write JSON".into());
    assert!(err.suggestion().unwrap().contains("writable"));
}

#[test]
fn engine_has_suggestion() {
    let err = MagistrError::Engine("connection lost".into());
    assert!(err.suggestion().unwrap().contains("backend"));
}

#[test]
fn model_has_suggestion() {
    let err = MagistrError::Model("unknown model".into());
    assert!(err.suggestion().unwrap().contains("magistr models"));
}

#[test]
fn permission_has_suggestion() {
    let err = MagistrError::Permission("audit read failed".into());
    assert!(err.suggestion().unwrap().contains("permissions"));
}

#[test]
fn sqlite_has_suggestion() {
    let err = MagistrError::sqlite("failed to open database", "unable to open file");
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("database"));
}

#[test]
fn sqlite_display_includes_context() {
    let err = MagistrError::sqlite("failed to insert session", "table locked");
    let msg = err.to_string();
    assert!(msg.contains("failed to insert session"), "got: {msg}");
    assert!(msg.contains("table locked"), "got: {msg}");
}

#[test]
fn sqlite_helper_builds_correctly() {
    let err = MagistrError::sqlite("open db", "disk I/O error");
    match &err {
        MagistrError::Sqlite { context, cause } => {
            assert_eq!(context, "open db");
            assert_eq!(cause, "disk I/O error");
        }
        _ => panic!("expected Sqlite variant"),
    }
}

#[test]
fn parse_has_suggestion() {
    let err = MagistrError::parse("JSON", "invalid costs file", "unexpected token");
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("syntax errors"));
}

#[test]
fn parse_display_includes_format_and_context() {
    let err = MagistrError::parse("TOML", "plan file", "expected '='");
    let msg = err.to_string();
    assert!(msg.contains("TOML"), "got: {msg}");
    assert!(msg.contains("plan file"), "got: {msg}");
    assert!(msg.contains("expected '='"), "got: {msg}");
}

#[test]
fn parse_helper_builds_correctly() {
    let err = MagistrError::parse("JSON", "config", "bad token");
    match &err {
        MagistrError::Parse {
            format,
            context,
            cause,
        } => {
            assert_eq!(format, "JSON");
            assert_eq!(context, "config");
            assert_eq!(cause, "bad token");
        }
        _ => panic!("expected Parse variant"),
    }
}

#[test]
fn sync_lock_has_suggestion() {
    let err = MagistrError::SyncLock("timed out".into());
    let suggestion = err.suggestion().unwrap();
    assert!(suggestion.contains("sync operation"), "got: {suggestion}");
}

#[test]
fn sync_lock_display() {
    let err = MagistrError::SyncLock("timed out waiting for lock".into());
    let msg = err.to_string();
    assert!(msg.contains("sync lock error"), "got: {msg}");
    assert!(msg.contains("timed out"), "got: {msg}");
}

#[test]
fn task_join_has_no_suggestion() {
    let err = MagistrError::TaskJoin("task panicked".into());
    assert!(err.suggestion().is_none());
}

#[test]
fn acp_auth_error_has_no_redundant_suggestion() {
    let err =
        MagistrError::Acp("Transport error: ACP prompt failed: Authentication required".into());
    assert!(
        err.suggestion().is_none(),
        "auth errors carry guidance in the message itself"
    );
}

#[test]
fn acp_binary_not_found_has_install_suggestion() {
    let err = MagistrError::Acp(
        "Claude Code selected as default provider but `claude` binary not found on PATH.".into(),
    );
    let suggestion = err.suggestion().unwrap();
    assert!(
        suggestion.contains("claude.ai/install.sh"),
        "should name the real installer: {suggestion}"
    );
}

#[test]
fn acp_no_agent_found_has_install_suggestion() {
    let err = MagistrError::Acp("ACP binary not found: No ACP-compatible agent found.".into());
    assert!(err.suggestion().unwrap().contains("Install with"));
}

#[test]
fn acp_spawn_no_such_file_has_install_suggestion() {
    let err = MagistrError::Acp(
        "Could not connect to the agent: Connection error: Failed to spawn ACP agent \
         `/opt/bin/claude`: No such file or directory (os error 2)"
            .into(),
    );
    assert!(err.suggestion().unwrap().contains("Install with"));
}

#[test]
fn acp_session_refusal_points_at_the_agent_reason_not_install() {
    let err = MagistrError::Acp(
        "Could not connect to the agent: Connection error: ACP session/new failed: \
         Invalid permissions.defaultMode: auto."
            .into(),
    );
    let suggestion = err.suggestion().expect("a handshake refusal gets guidance");
    assert!(
        suggestion.contains("settings.json"),
        "should point at the agent's own settings: {suggestion}"
    );
    assert!(
        suggestion.contains("CLAUDE_CONFIG_DIR"),
        "should warn about the config-dir override: {suggestion}"
    );
    assert!(
        !suggestion.contains("npm install"),
        "an installed agent must not get the install hint: {suggestion}"
    );
}

#[test]
fn acp_agent_exit_points_at_the_agent_reason_not_install() {
    let err = MagistrError::Acp(
        "Connection error: ACP agent exited during initialize (exit code: 3)\n  agent stderr:\n    boom"
            .into(),
    );
    let suggestion = err.suggestion().expect("an early exit gets guidance");
    assert!(!suggestion.contains("npm install"), "{suggestion}");
    assert!(suggestion.contains("stderr"), "{suggestion}");
}

#[test]
fn acp_generic_error_has_no_install_suggestion() {
    let err = MagistrError::Acp("Transport error: ACP prompt failed: boom".into());
    assert!(
        err.suggestion().is_none(),
        "a running agent's runtime error is not an install problem"
    );
}

#[test]
fn acp_unauthorized_error_has_no_redundant_suggestion() {
    let err = MagistrError::Acp("unauthorized".into());
    assert!(err.suggestion().is_none());
}

#[test]
fn acp_invalid_api_key_has_no_redundant_suggestion() {
    let err = MagistrError::Acp("invalid x-api-key".into());
    assert!(err.suggestion().is_none());
}

#[test]
fn acp_no_auth_configured_has_no_redundant_suggestion() {
    let err = MagistrError::Acp(
        "No authentication configured for Claude API.\n\n  Option 1 (recommended): Log in..."
            .into(),
    );
    assert!(err.suggestion().is_none());
}

// ── From impls ────────────────────────────────────────────────────

#[test]
fn from_serde_json_error_produces_serialize_variant() {
    let json_err: serde_json::Error = serde_json::from_str::<String>("not json").unwrap_err();
    let err: MagistrError = json_err.into();
    assert!(
        matches!(err, MagistrError::Serialize(_)),
        "expected Serialize, got: {err:?}"
    );
    assert!(err.to_string().contains("serialization error"));
}

#[test]
fn from_fmt_error_produces_serialize_variant() {
    let fmt_err = std::fmt::Error;
    let err: MagistrError = fmt_err.into();
    assert!(
        matches!(err, MagistrError::Serialize(_)),
        "expected Serialize, got: {err:?}"
    );
}

#[test]
fn serde_json_error_auto_converts_with_question_mark() {
    fn fallible() -> Result<()> {
        let _: String = serde_json::from_str("bad json")?;
        Ok(())
    }
    let err = fallible().unwrap_err();
    assert!(matches!(err, MagistrError::Serialize(_)));
}

// ── with_context extension ────────────────────────────────────────

#[test]
fn with_context_converts_error_to_target_variant() {
    let result: std::result::Result<(), String> = Err("something broke".into());
    let mapped = result.with_context(MagistrError::Plugin);
    let err = mapped.unwrap_err();
    assert!(matches!(err, MagistrError::Plugin(ref msg) if msg.contains("something broke")));
}

#[test]
fn with_context_passes_through_ok() {
    let result: std::result::Result<i32, String> = Ok(42);
    let mapped = result.with_context(MagistrError::Plugin);
    assert_eq!(mapped.unwrap(), 42);
}

#[test]
fn with_context_works_with_io_error() {
    let result: std::result::Result<(), std::io::Error> = Err(std::io::Error::other("disk full"));
    let mapped = result.with_context(MagistrError::Engine);
    let err = mapped.unwrap_err();
    assert!(matches!(err, MagistrError::Engine(ref msg) if msg.contains("disk full")));
}
