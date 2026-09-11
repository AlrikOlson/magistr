use magistr_workspace::config::{
    CommandAdmission, DetectedCommand, ResolveOptions, ResolvedConfig,
};
use serde_json::{Value, json};

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../subsystems/workspace/conformance/config.json"
    ))
    .unwrap()
}

#[test]
fn shared_config_resolution_corpus() {
    for fixture in corpus()["cases"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let mut options = ResolveOptions::new(root.path());
        for (key, file) in [
            ("org", "org.toml"),
            ("user", "user.toml"),
            ("workspace", "workbench.toml"),
        ] {
            if let Some(body) = fixture[key].as_str() {
                std::fs::write(root.path().join(file), body).unwrap();
            }
        }
        options.env.insert(
            "WORKBENCH_ORG_POLICY".into(),
            root.path().join("org.toml").display().to_string(),
        );
        options.env.insert(
            "WORKBENCH_USER_CONFIG".into(),
            root.path().join("user.toml").display().to_string(),
        );
        options.session_flags = fixture["session"].as_object().cloned().unwrap_or_default();
        let result = ResolvedConfig::resolve(&options);
        if fixture["unsupported"] == true {
            assert!(result.is_err(), "{}", fixture["name"]);
            continue;
        }
        let config = result.unwrap();
        for (key, expected) in fixture["effective"].as_object().unwrap() {
            assert_eq!(
                &config.effective[key], expected,
                "{}: {key}",
                fixture["name"]
            );
        }
        assert_eq!(
            json!(config.commands),
            fixture["commands"],
            "{}",
            fixture["name"]
        );
        let violations: Vec<_> = config
            .violations
            .iter()
            .map(|violation| json!([violation.key, violation.attempted_by, violation.pinned_by]))
            .collect();
        assert_eq!(
            json!(violations),
            fixture["violations"],
            "{}",
            fixture["name"]
        );
        let fragments = fixture["warnings"].as_array().unwrap();
        assert_eq!(
            config.warnings.len(),
            fragments.len(),
            "{}",
            fixture["name"]
        );
        for (warning, fragment) in config.warnings.iter().zip(fragments) {
            assert!(warning.contains(fragment.as_str().unwrap()), "{warning}");
        }
        for (key, expected) in fixture["trace"].as_object().unwrap() {
            let entry = config.trace.iter().find(|entry| &entry.key == key).unwrap();
            assert_eq!(
                json!([entry.decided_by, entry.pinned_by]),
                *expected,
                "{}: {key}",
                fixture["name"]
            );
        }
        assert!(config.doctor(root.path()).contains("security.redaction ="));
    }
}

#[test]
fn shared_command_admission_corpus() {
    for fixture in corpus()["guards"].as_array().unwrap() {
        let mut config = ResolvedConfig::default();
        config
            .commands
            .insert("test".into(), fixture["line"].as_str().unwrap().into());
        let args = fixture["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().into())
            .collect::<Vec<_>>();
        let admitted = CommandAdmission::admit(&config, "test", &args);
        assert_eq!(
            admitted.is_ok(),
            fixture["allowed"].as_bool().unwrap(),
            "{}",
            fixture["line"]
        );
        if fixture["argv"].is_array() {
            assert_eq!(json!(admitted.unwrap()), fixture["argv"]);
        }
    }
}

#[test]
fn shared_command_detection_corpus() {
    for fixture in corpus()["detection"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        for (file, body) in fixture["files"].as_object().unwrap() {
            let path = root.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body.as_str().unwrap()).unwrap();
        }
        let commands = DetectedCommand::detect(root.path());
        assert_eq!(json!(commands), fixture["commands"], "{}", fixture["name"]);
        let rendered = DetectedCommand::render_table(&commands);
        assert!(rendered.parse::<toml::Table>().is_ok());
        assert!(!root.path().join("workbench.toml").exists());
    }
}
