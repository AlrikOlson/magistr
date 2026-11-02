use super::*;
use crate::decision::config::OutputFormat;

fn resolve(entries: &[(&str, &str)]) -> DecisionConfigResolution {
    DecisionConfigResolution::from_lookup(
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
        ("MAGISTR_WORKFLOW_STRICT_MODE", "true"),
        ("MAX_HISTORY_SIZE", " 240 "),
        ("MAGISTR_WORKFLOW_OUTPUT_FORMAT", "JSON"),
        ("MAGISTR_WORKFLOW_NO_COLOR", "true"),
        ("MAGISTR_WORKFLOW_LOG_TIMEOUT", "42"),
        ("MAGISTR_WORKFLOW_MAX_THREAD_DEPTH", "8"),
        ("MAGISTR_WORKFLOW_RECENT_NOTES_LIMIT", "7"),
        ("MAGISTR_WORKFLOW_ROLLUP_MAX_ITEMS", "19"),
        ("MAGISTR_WORKFLOW_BROADCAST_PATH", " /injected/socket "),
    ]);
    let config = &resolved.config;
    assert!(config.validation.strict_mode);
    assert!(config.validation.require_thought_prefix);
    assert!(config.validation.require_rationale_prefix);
    assert!(!config.validation.allow_custom_purpose);
    assert_eq!(config.system.max_history_size, 240);
    assert_eq!(config.display.output_format, OutputFormat::Json);
    assert!(!config.display.color_output);
    assert_eq!(config.system.log_timeout, 42);
    assert_eq!(config.system.max_thread_depth, 8);
    assert_eq!(config.system.recent_notes_limit, 7);
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
            ConfigSource::Environment("MAGISTR_WORKFLOW_STRICT_MODE")
        );
    }
    assert_eq!(
        resolved.sources["system.max_history_size"],
        ConfigSource::Environment("MAX_HISTORY_SIZE")
    );
    assert_eq!(
        resolved.sources["features.enable_threading"],
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
        ("MAGISTR_WORKFLOW_STRICT_MODE", "TRUE"),
        ("MAX_HISTORY_SIZE", "0"),
        ("MAGISTR_WORKFLOW_OUTPUT_FORMAT", "sensitive-invalid-value"),
        ("MAGISTR_WORKFLOW_LOG_TIMEOUT", "-1"),
        ("MAGISTR_WORKFLOW_MAX_THREAD_DEPTH", "4294967296"),
        ("MAGISTR_WORKFLOW_RECENT_NOTES_LIMIT", " "),
        ("MAGISTR_WORKFLOW_ROLLUP_MAX_ITEMS", "nan"),
        ("MAGISTR_WORKFLOW_BROADCAST_PATH", "  "),
        ("MAGISTR_WORKFLOW_DEFAULT_LOG_ID", "../../sensitive-log"),
    ]);
    assert!(!resolved.config.validation.strict_mode);
    assert_eq!(resolved.config.system.max_history_size, 100);
    assert_eq!(resolved.config.system.log_timeout, 60);
    assert_eq!(resolved.config.system.max_thread_depth, 5);
    assert_eq!(resolved.config.system.recent_notes_limit, 3);
    assert_eq!(resolved.config.system.rollup_max_items, 12);
    assert_eq!(resolved.config.display.output_format, OutputFormat::Console);
    assert_eq!(resolved.config.broadcast.path, None);
    assert!(!resolved.config.features.enable_logs);
    assert_eq!(resolved.config.features.default_log_id, None);
    assert_eq!(
        resolved.sources["display.output_format"],
        ConfigSource::Default
    );
    assert_eq!(
        resolved.sources["features.default_log_id"],
        ConfigSource::Default
    );
    assert!(!format!("{:?}", resolved.sources).contains("sensitive"));
}

#[test]
fn automatic_log_identity_waits_for_project_admission() {
    for variable in ["MAGISTR_WORKFLOW_ENABLE_LOGS", "MAGISTR_WORKFLOW_AUTO_LOG"] {
        let resolved = resolve(&[(variable, "true")]);
        assert!(resolved.config.features.enable_logs);
        assert_eq!(resolved.config.features.default_log_id, None);
        assert_eq!(
            resolved.sources["features.default_log_id"],
            ConfigSource::ProjectIdentity
        );
        assert_eq!(
            resolved.sources["features.enable_logs"],
            ConfigSource::Environment(variable)
        );
    }
}

#[test]
fn explicit_log_id_wins_and_enables_logs() {
    let resolved = resolve(&[
        ("MAGISTR_WORKFLOW_AUTO_LOG", "true"),
        ("MAGISTR_WORKFLOW_ENABLE_LOGS", "false"),
        ("MAGISTR_WORKFLOW_DEFAULT_LOG_ID", " explicit-log_2 "),
    ]);
    assert!(resolved.config.features.enable_logs);
    assert_eq!(
        resolved.config.features.default_log_id.as_deref(),
        Some("explicit-log_2")
    );
    assert_eq!(
        resolved.sources["features.default_log_id"],
        ConfigSource::Environment("MAGISTR_WORKFLOW_DEFAULT_LOG_ID")
    );
    assert_eq!(
        resolved.sources["features.enable_logs"],
        ConfigSource::Environment("MAGISTR_WORKFLOW_DEFAULT_LOG_ID")
    );

    let rejected = resolve(&[
        ("MAGISTR_WORKFLOW_AUTO_LOG", "true"),
        ("MAGISTR_WORKFLOW_DEFAULT_LOG_ID", ".."),
    ]);
    assert!(rejected.config.features.enable_logs);
    assert_eq!(rejected.config.features.default_log_id, None);
    assert_eq!(
        rejected.sources["features.default_log_id"],
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
            ("MAGISTR_WORKFLOW_OUTPUT_FORMAT", "private-format-payload"),
            ("MAGISTR_WORKFLOW_DEFAULT_LOG_ID", "../private-log-payload"),
        ])
    });
    let output = std::fs::read_to_string(log.path()).expect("diagnostics");
    assert!(output.contains("MAGISTR_WORKFLOW_OUTPUT_FORMAT"));
    assert!(output.contains("MAGISTR_WORKFLOW_DEFAULT_LOG_ID"));
    assert!(!output.contains("private-format-payload"));
    assert!(!output.contains("private-log-payload"));
}

#[test]
fn empty_inputs_do_not_inherit_ambient_environment() {
    const CHILD: &str = "MAGISTR_TEST_DECISION_EMPTY_INPUTS";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "decision::config_resolution::tests::empty_inputs_do_not_inherit_ambient_environment",
            ])
            .env(CHILD, "1")
            .env("MAGISTR_WORKFLOW_STRICT_MODE", "true")
            .env("MAGISTR_WORKFLOW_ENABLE_LOGS", "true")
            .env("MAGISTR_WORKFLOW_DEFAULT_LOG_ID", "ambient-log")
            .env("MAGISTR_WORKFLOW_DATA_DIR", "/ambient-store")
            .status()
            .expect("isolated environment check");
        assert!(status.success(), "isolated environment check failed");
        return;
    }
    let resolved = resolve(&[]);
    assert!(!resolved.config.validation.strict_mode);
    assert!(!resolved.config.features.enable_logs);
    assert_eq!(resolved.config.features.default_log_id, None);
    assert_eq!(
        resolved.config.persistence.data_dir,
        std::path::Path::new("injected-store")
    );
    assert_eq!(
        resolved.sources["features.default_log_id"],
        ConfigSource::Default
    );
}

#[test]
fn supplied_persistence_is_authoritative() {
    let resolved = resolve(&[
        ("MAGISTR_WORKFLOW_PERSIST", "false"),
        ("MAGISTR_WORKFLOW_DATA_DIR", "/other-store"),
    ]);
    assert!(resolved.config.persistence.enabled);
    assert_eq!(
        resolved.config.persistence.data_dir,
        std::path::Path::new("injected-store")
    );
}
