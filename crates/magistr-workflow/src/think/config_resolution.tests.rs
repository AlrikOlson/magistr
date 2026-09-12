use super::*;
use crate::think::config::OutputFormat;

fn resolve(entries: &[(&str, &str)]) -> ThinkConfigResolution {
    ThinkConfigResolution::from_lookup(
        |key| {
            entries
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        },
        PersistenceConfig {
            enabled: true,
            data_dir: "injected-store".into(),
        },
    )
}

#[test]
fn injected_settings_resolve_with_field_provenance() {
    let resolved = resolve(&[
        ("THINK_AND_SHIP_STRICT_MODE", "true"),
        ("MAX_HISTORY_SIZE", " 240 "),
        ("THINK_AND_SHIP_OUTPUT_FORMAT", "JSON"),
        ("THINK_AND_SHIP_NO_COLOR", "true"),
        ("THINK_AND_SHIP_SESSION_TIMEOUT", "42"),
        ("THINK_AND_SHIP_MAX_BRANCH_DEPTH", "8"),
        ("THINK_AND_SHIP_RECENT_STEPS_LIMIT", "7"),
        ("THINK_AND_SHIP_ROLLUP_MAX_ITEMS", "19"),
        ("THINK_AND_SHIP_BROADCAST_PATH", " /injected/socket "),
    ]);
    let config = &resolved.config;
    assert!(config.validation.strict_mode);
    assert!(config.validation.require_thought_prefix);
    assert!(config.validation.require_rationale_prefix);
    assert!(!config.validation.allow_custom_purpose);
    assert_eq!(config.system.max_history_size, 240);
    assert_eq!(config.display.output_format, OutputFormat::Json);
    assert!(!config.display.color_output);
    assert_eq!(config.system.session_timeout, 42);
    assert_eq!(config.system.max_branch_depth, 8);
    assert_eq!(config.system.recent_steps_limit, 7);
    assert_eq!(config.system.rollup_max_items, 19);
    assert_eq!(
        config.broadcast.path.as_deref(),
        Some(std::path::Path::new("/injected/socket"))
    );
    for field in [
        "validation.strict_mode",
        "validation.require_thought_prefix",
        "validation.require_rationale_prefix",
        "validation.allow_custom_purpose",
    ] {
        assert_eq!(
            resolved.sources[field],
            ConfigSource::Environment("THINK_AND_SHIP_STRICT_MODE")
        );
    }
    assert_eq!(
        resolved.sources["system.max_history_size"],
        ConfigSource::Environment("MAX_HISTORY_SIZE")
    );
    assert_eq!(
        resolved.sources["features.enable_branching"],
        ConfigSource::Default
    );
    assert_eq!(
        resolved.sources["persistence.enabled"],
        ConfigSource::Explicit
    );
    assert_eq!(
        resolved.sources["persistence.data_dir"],
        ConfigSource::Explicit
    );
}

#[test]
fn invalid_and_blank_inputs_preserve_defaults_and_do_not_enter_provenance() {
    let resolved = resolve(&[
        ("THINK_AND_SHIP_STRICT_MODE", "TRUE"),
        ("MAX_HISTORY_SIZE", "0"),
        ("THINK_AND_SHIP_OUTPUT_FORMAT", "sensitive-invalid-value"),
        ("THINK_AND_SHIP_SESSION_TIMEOUT", "-1"),
        ("THINK_AND_SHIP_MAX_BRANCH_DEPTH", "4294967296"),
        ("THINK_AND_SHIP_RECENT_STEPS_LIMIT", " "),
        ("THINK_AND_SHIP_ROLLUP_MAX_ITEMS", "nan"),
        ("THINK_AND_SHIP_BROADCAST_PATH", "  "),
        (
            "THINK_AND_SHIP_DEFAULT_SESSION_ID",
            "../../sensitive-session",
        ),
    ]);
    assert!(!resolved.config.validation.strict_mode);
    assert_eq!(resolved.config.system.max_history_size, 100);
    assert_eq!(resolved.config.system.session_timeout, 60);
    assert_eq!(resolved.config.system.max_branch_depth, 5);
    assert_eq!(resolved.config.system.recent_steps_limit, 3);
    assert_eq!(resolved.config.system.rollup_max_items, 12);
    assert_eq!(resolved.config.display.output_format, OutputFormat::Console);
    assert_eq!(resolved.config.broadcast.path, None);
    assert!(!resolved.config.features.enable_sessions);
    assert_eq!(resolved.config.features.default_session_id, None);
    assert_eq!(
        resolved.sources["display.output_format"],
        ConfigSource::Default
    );
    assert_eq!(
        resolved.sources["features.default_session_id"],
        ConfigSource::Default
    );
    assert!(!format!("{:?}", resolved.sources).contains("sensitive"));
}

#[test]
fn automatic_session_identity_waits_for_project_admission() {
    for variable in [
        "THINK_AND_SHIP_ENABLE_SESSIONS",
        "THINK_AND_SHIP_AUTO_SESSION",
    ] {
        let resolved = resolve(&[(variable, "true")]);
        assert!(resolved.config.features.enable_sessions);
        assert_eq!(resolved.config.features.default_session_id, None);
        assert_eq!(
            resolved.sources["features.default_session_id"],
            ConfigSource::ProjectIdentity
        );
        assert_eq!(
            resolved.sources["features.enable_sessions"],
            ConfigSource::Environment(variable)
        );
    }
}

#[test]
fn explicit_session_id_wins_and_enables_sessions() {
    let resolved = resolve(&[
        ("THINK_AND_SHIP_AUTO_SESSION", "true"),
        ("THINK_AND_SHIP_ENABLE_SESSIONS", "false"),
        ("THINK_AND_SHIP_DEFAULT_SESSION_ID", " explicit-session_2 "),
    ]);
    assert!(resolved.config.features.enable_sessions);
    assert_eq!(
        resolved.config.features.default_session_id.as_deref(),
        Some("explicit-session_2")
    );
    assert_eq!(
        resolved.sources["features.default_session_id"],
        ConfigSource::Environment("THINK_AND_SHIP_DEFAULT_SESSION_ID")
    );
    assert_eq!(
        resolved.sources["features.enable_sessions"],
        ConfigSource::Environment("THINK_AND_SHIP_DEFAULT_SESSION_ID")
    );

    let rejected = resolve(&[
        ("THINK_AND_SHIP_AUTO_SESSION", "true"),
        ("THINK_AND_SHIP_DEFAULT_SESSION_ID", ".."),
    ]);
    assert!(rejected.config.features.enable_sessions);
    assert_eq!(rejected.config.features.default_session_id, None);
    assert_eq!(
        rejected.sources["features.default_session_id"],
        ConfigSource::ProjectIdentity
    );
}

#[test]
fn rejected_values_are_absent_from_diagnostics() {
    let log = tempfile::NamedTempFile::new().expect("diagnostic log");
    let handle = log.reopen().expect("log writer");
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_writer(move || handle.try_clone().expect("cloned log writer"))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        resolve(&[
            ("THINK_AND_SHIP_OUTPUT_FORMAT", "private-format-payload"),
            (
                "THINK_AND_SHIP_DEFAULT_SESSION_ID",
                "../private-session-payload",
            ),
        ])
    });
    let output = std::fs::read_to_string(log.path()).expect("diagnostics");
    assert!(output.contains("THINK_AND_SHIP_OUTPUT_FORMAT"));
    assert!(output.contains("THINK_AND_SHIP_DEFAULT_SESSION_ID"));
    assert!(!output.contains("private-format-payload"));
    assert!(!output.contains("private-session-payload"));
}

#[test]
fn empty_inputs_do_not_inherit_ambient_environment() {
    const CHILD: &str = "MAGISTR_TEST_THINK_EMPTY_INPUTS";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "think::config_resolution::tests::empty_inputs_do_not_inherit_ambient_environment",
            ])
            .env(CHILD, "1")
            .env("THINK_AND_SHIP_STRICT_MODE", "true")
            .env("THINK_AND_SHIP_ENABLE_SESSIONS", "true")
            .env("THINK_AND_SHIP_DEFAULT_SESSION_ID", "ambient-session")
            .env("THINK_AND_SHIP_DATA_DIR", "/ambient-store")
            .status()
            .expect("isolated environment check");
        assert!(status.success(), "isolated environment check failed");
        return;
    }
    let resolved = resolve(&[]);
    assert!(!resolved.config.validation.strict_mode);
    assert!(!resolved.config.features.enable_sessions);
    assert_eq!(resolved.config.features.default_session_id, None);
    assert_eq!(
        resolved.config.persistence.data_dir,
        std::path::Path::new("injected-store")
    );
    assert_eq!(
        resolved.sources["features.default_session_id"],
        ConfigSource::Default
    );
}

#[test]
fn supplied_persistence_is_authoritative() {
    let resolved = resolve(&[
        ("THINK_AND_SHIP_PERSIST", "false"),
        ("THINK_AND_SHIP_DATA_DIR", "/other-store"),
    ]);
    assert!(resolved.config.persistence.enabled);
    assert_eq!(
        resolved.config.persistence.data_dir,
        std::path::Path::new("injected-store")
    );
}
