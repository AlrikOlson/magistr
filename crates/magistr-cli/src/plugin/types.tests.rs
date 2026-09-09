use super::*;

// ── PluginKind tests ───────────────────────────────────────

#[test]
fn kind_label() {
    assert_eq!(PluginKind::Detection.label(), "detection");
    assert_eq!(PluginKind::Template.label(), "template");
    assert_eq!(PluginKind::WorkflowStep.label(), "workflow-step");
    assert_eq!(PluginKind::QualityGate.label(), "quality-gate");
}

#[test]
fn kind_display() {
    assert_eq!(format!("{}", PluginKind::Detection), "detection");
    assert_eq!(format!("{}", PluginKind::Template), "template");
    assert_eq!(format!("{}", PluginKind::WorkflowStep), "workflow-step");
    assert_eq!(format!("{}", PluginKind::QualityGate), "quality-gate");
}

#[test]
fn kind_is_executable() {
    assert!(!PluginKind::Detection.is_executable());
    assert!(!PluginKind::Template.is_executable());
    assert!(PluginKind::WorkflowStep.is_executable());
    assert!(PluginKind::QualityGate.is_executable());
}

/// The retired `backend` kind is no variant at all — the enum cannot name
/// what the bridge no longer carries.
#[test]
fn kind_backend_is_not_a_variant() {
    #[derive(Debug, Deserialize)]
    struct W {
        kind: PluginKind,
    }
    match toml::from_str::<W>("kind = \"backend\"") {
        Ok(w) => panic!("the bridge kind parsed as {:?}", w.kind),
        Err(e) => assert!(e.to_string().contains("unknown variant"), "{e}"),
    }
}

#[test]
fn kind_serde_round_trip() {
    for kind in [
        PluginKind::Detection,
        PluginKind::Template,
        PluginKind::WorkflowStep,
        PluginKind::QualityGate,
    ] {
        let toml_str = format!("kind = \"{}\"", kind.label());
        #[derive(Deserialize)]
        struct W {
            kind: PluginKind,
        }
        let w: W = toml::from_str(&toml_str).unwrap();
        assert_eq!(w.kind, kind);
    }
}

// ── PluginSource tests ─────────────────────────────────────

#[test]
fn source_local_display() {
    let src = PluginSource::Local {
        path: "/home/user/plugins/foo".to_string(),
    };
    assert_eq!(src.to_string(), "local:/home/user/plugins/foo");
}

#[test]
fn source_git_display_no_ref() {
    let src = PluginSource::Git {
        url: "https://github.com/team/plugin.git".to_string(),
        r#ref: None,
    };
    assert_eq!(src.to_string(), "git:https://github.com/team/plugin.git");
}

#[test]
fn source_git_display_with_ref() {
    let src = PluginSource::Git {
        url: "https://github.com/team/plugin.git".to_string(),
        r#ref: Some("v1.0".to_string()),
    };
    assert_eq!(
        src.to_string(),
        "git:https://github.com/team/plugin.git@v1.0"
    );
}

// ── PluginManifest tests ───────────────────────────────────

#[test]
fn manifest_from_toml_valid() {
    let toml = r#"
            name = "my-plugin"
            version = "1.0.0"
            description = "A test plugin"
            kind = "detection"
            author = "Test Author"
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.name, "my-plugin");
    assert_eq!(manifest.version, "1.0.0");
    assert_eq!(manifest.description.as_deref(), Some("A test plugin"));
    assert_eq!(manifest.kind, PluginKind::Detection);
    assert_eq!(manifest.author.as_deref(), Some("Test Author"));
}

#[test]
fn manifest_from_toml_minimal() {
    let toml = r#"
            name = "minimal"
            version = "0.1.0"
            kind = "template"
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.name, "minimal");
    assert_eq!(manifest.description, None);
    assert_eq!(manifest.author, None);
    assert_eq!(manifest.kind, PluginKind::Template);
}

#[test]
fn manifest_from_toml_invalid() {
    let result = PluginManifest::from_toml("not valid toml {{{");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("parse"));
}

#[test]
fn manifest_from_toml_missing_required_field() {
    let toml = r#"
            name = "incomplete"
            version = "1.0.0"
        "#;
    // Missing `kind` field
    let result = PluginManifest::from_toml(toml);
    assert!(result.is_err());
}

#[test]
fn manifest_round_trip() {
    let manifest = PluginManifest {
        name: "round-trip".to_string(),
        version: "2.0.0".to_string(),
        description: Some("Round trip test".to_string()),
        kind: PluginKind::Detection,
        author: Some("Tester".to_string()),
        entrypoint: None,
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };

    let toml_str = manifest.to_toml().unwrap();
    let parsed = PluginManifest::from_toml(&toml_str).unwrap();
    assert_eq!(parsed, manifest);
}

/// The plugin bridge no longer reaches a model: a manifest still declaring
/// the retired `backend` kind is refused, and the refusal says where a
/// model is reached now.
#[test]
fn manifest_backend_kind_is_refused_naming_the_acp_agent() {
    let toml = r#"
            name = "openai-backend"
            version = "1.0.0"
            kind = "backend"
            entrypoint = "bin/openai-backend"
        "#;
    let err = PluginManifest::from_toml(toml).unwrap_err().to_string();
    assert!(err.contains("magistr acp-agent"), "{err}");
    assert!(err.contains("\"backend\""), "{err}");
    assert!(err.contains("retired"), "{err}");
}

/// An unknown kind is still the parser's ordinary refusal, not the
/// backend pointer.
#[test]
fn manifest_unknown_kind_is_a_parse_error() {
    let toml = r#"
            name = "x"
            version = "1.0.0"
            kind = "telemetry"
        "#;
    let err = PluginManifest::from_toml(toml).unwrap_err().to_string();
    assert!(err.contains("parse"), "{err}");
    assert!(!err.contains("magistr acp-agent"), "{err}");
}

#[test]
fn manifest_executable_plugin_full() {
    let toml = r#"
            name = "sonarqube-gate"
            version = "1.0.0"
            description = "SonarQube quality gate"
            kind = "quality-gate"
            author = "magistr-plugins"
            entrypoint = "bin/sonarqube-gate"
            permissions = ["network", "env-read"]

            [[dependencies]]
            name = "auth-helper"
            version = ">=1.0.0"

            [compatibility]
            jig_version = "0.2.0"
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.name, "sonarqube-gate");
    assert_eq!(manifest.kind, PluginKind::QualityGate);
    assert_eq!(manifest.entrypoint.as_deref(), Some("bin/sonarqube-gate"));
    assert_eq!(
        manifest.permissions,
        vec![PluginPermission::Network, PluginPermission::EnvRead]
    );
    assert_eq!(manifest.dependencies.len(), 1);
    assert_eq!(manifest.dependencies[0].name, "auth-helper");
    assert_eq!(manifest.dependencies[0].version.as_deref(), Some(">=1.0.0"));
    assert_eq!(manifest.compatibility.jig_version.as_deref(), Some("0.2.0"));
}

#[test]
fn manifest_quality_gate_plugin() {
    let toml = r#"
            name = "sonarqube-gate"
            version = "0.5.0"
            kind = "quality-gate"
            entrypoint = "bin/sonar-gate"
            permissions = ["network", "file-read"]
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.kind, PluginKind::QualityGate);
    assert_eq!(manifest.entrypoint.as_deref(), Some("bin/sonar-gate"));
    assert_eq!(manifest.permissions.len(), 2);
}

#[test]
fn manifest_workflow_step_plugin() {
    let toml = r#"
            name = "pre-review"
            version = "1.0.0"
            kind = "workflow-step"
            entrypoint = "bin/pre-review"
            permissions = ["file-read", "shell-exec"]
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.kind, PluginKind::WorkflowStep);
    assert!(manifest.kind.is_executable());
}

#[test]
fn manifest_executable_round_trip() {
    let manifest = PluginManifest {
        name: "my-backend".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::QualityGate,
        author: None,
        entrypoint: Some("bin/my-backend".to_string()),
        permissions: vec![PluginPermission::Network, PluginPermission::EnvRead],
        dependencies: vec![PluginDependency {
            name: "core-lib".to_string(),
            version: Some("^1.0".to_string()),
        }],
        compatibility: PluginCompatibility {
            jig_version: Some("0.2.0".to_string()),
            rust_edition: None,
        },
    };

    let toml_str = manifest.to_toml().unwrap();
    let parsed = PluginManifest::from_toml(&toml_str).unwrap();
    assert_eq!(parsed, manifest);
}

// ── Manifest validation tests ──────────────────────────────

#[test]
fn validate_executable_without_entrypoint() {
    let manifest = PluginManifest {
        name: "no-entry".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::QualityGate,
        author: None,
        entrypoint: None,
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };
    let errors = manifest.validate();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("requires an entrypoint"));
}

#[test]
fn validate_declarative_with_entrypoint() {
    let manifest = PluginManifest {
        name: "bad-detect".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::Detection,
        author: None,
        entrypoint: Some("bin/oops".to_string()),
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };
    let errors = manifest.validate();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("should not have an entrypoint"));
}

#[test]
fn validate_valid_executable() {
    let manifest = PluginManifest {
        name: "good-backend".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::QualityGate,
        author: None,
        entrypoint: Some("bin/backend".to_string()),
        permissions: vec![PluginPermission::Network],
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };
    assert!(manifest.validate().is_empty());
}

#[test]
fn validate_valid_declarative() {
    let manifest = PluginManifest {
        name: "good-detect".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::Detection,
        author: None,
        entrypoint: None,
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };
    assert!(manifest.validate().is_empty());
}

// ── PluginPermission tests ─────────────────────────────────

#[test]
fn permission_label_and_display() {
    assert_eq!(PluginPermission::FileRead.label(), "file-read");
    assert_eq!(PluginPermission::FileWrite.label(), "file-write");
    assert_eq!(PluginPermission::Network.label(), "network");
    assert_eq!(PluginPermission::EnvRead.label(), "env-read");
    assert_eq!(PluginPermission::ShellExec.label(), "shell-exec");
    assert_eq!(format!("{}", PluginPermission::Network), "network");
}

// ── PluginDependency tests ─────────────────────────────────

#[test]
fn dependency_optional_version() {
    let toml = r#"
            name = "test"
            version = "1.0.0"
            kind = "quality-gate"
            entrypoint = "bin/test"

            [[dependencies]]
            name = "dep-a"

            [[dependencies]]
            name = "dep-b"
            version = ">=2.0"
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.dependencies.len(), 2);
    assert!(manifest.dependencies[0].version.is_none());
    assert_eq!(manifest.dependencies[1].version.as_deref(), Some(">=2.0"));
}

// ── PluginCompatibility tests ──────────────────────────────

#[test]
fn compatibility_defaults_empty() {
    let compat = PluginCompatibility::default();
    assert!(compat.jig_version.is_none());
    assert!(compat.rust_edition.is_none());
}

#[test]
fn compatibility_full() {
    let toml = r#"
            name = "compat-test"
            version = "1.0.0"
            kind = "workflow-step"
            entrypoint = "bin/test"

            [compatibility]
            jig_version = "0.3.0"
            rust_edition = "2024"
        "#;
    let manifest = PluginManifest::from_toml(toml).unwrap();
    assert_eq!(manifest.compatibility.jig_version.as_deref(), Some("0.3.0"));
    assert_eq!(manifest.compatibility.rust_edition.as_deref(), Some("2024"));
}

// ── DetectionRule tests ────────────────────────────────────

#[test]
fn detection_rule_defaults() {
    let toml = r#"
            [[rules]]
            name = "my-framework"
            file_patterns = ["my-framework.config.js"]
        "#;
    let config = DetectionConfig::from_toml(toml).unwrap();
    assert_eq!(config.rules.len(), 1);
    assert_eq!(config.rules[0].confidence, 80);
    assert_eq!(config.rules[0].category, "framework");
    assert!(config.rules[0].content_patterns.is_empty());
}

#[test]
fn detection_config_multiple_rules() {
    let toml = r#"
            [[rules]]
            name = "framework-a"
            file_patterns = ["a.config.js"]
            confidence = 90
            category = "framework"

            [[rules]]
            name = "tool-b"
            file_patterns = [".tool-b.yml"]
            content_patterns = ["toolb-version:"]
            confidence = 75
            category = "tool"
        "#;
    let config = DetectionConfig::from_toml(toml).unwrap();
    assert_eq!(config.rules.len(), 2);
    assert_eq!(config.rules[0].name, "framework-a");
    assert_eq!(config.rules[0].confidence, 90);
    assert_eq!(config.rules[1].name, "tool-b");
    assert_eq!(config.rules[1].content_patterns, vec!["toolb-version:"]);
}

// ── TemplateConfig tests ───────────────────────────────────

#[test]
fn template_config_from_toml() {
    let toml = r#"
            [[files]]
            src = "rules/custom.md"
            dest = "rules/custom.md"

            [[files]]
            src = "hooks/pre-commit.sh"
            dest = "hooks/pre-commit.sh"
        "#;
    let config = TemplateConfig::from_toml(toml).unwrap();
    assert_eq!(config.files.len(), 2);
    assert_eq!(config.files[0].src, "rules/custom.md");
    assert_eq!(config.files[0].dest, "rules/custom.md");
}

#[test]
fn template_config_empty() {
    let config = TemplateConfig::from_toml("").unwrap();
    assert!(config.files.is_empty());
}

// ── PluginError tests ──────────────────────────────────────

#[test]
fn error_display_messages() {
    let parse = PluginError::ManifestParse("bad syntax".to_string());
    assert!(parse.to_string().contains("parse plugin manifest"));

    let ser = PluginError::ManifestSerialize("encoding".to_string());
    assert!(ser.to_string().contains("serialize"));

    let not_found = PluginError::ManifestNotFound("/tmp/foo".to_string());
    assert!(not_found.to_string().contains(MANIFEST_FILENAME));

    let dir = PluginError::DirNotFound("/nope".to_string());
    assert!(dir.to_string().contains("not found"));

    let already = PluginError::AlreadyInstalled("dup".to_string());
    assert!(already.to_string().contains("already installed"));

    let missing = PluginError::NotFound("gone".to_string());
    assert!(missing.to_string().contains("not found"));

    let io = PluginError::Io(
        "/tmp".to_string(),
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    );
    assert!(io.to_string().contains("I/O error"));

    let git = PluginError::GitError("clone failed".to_string());
    assert!(git.to_string().contains("git error"));
}

// ── InstalledPlugin serialization ──────────────────────────

#[test]
fn installed_plugin_json_round_trip() {
    let plugin = InstalledPlugin {
        manifest: PluginManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            kind: PluginKind::Detection,
            author: None,
            entrypoint: None,
            permissions: Vec::new(),
            dependencies: Vec::new(),
            compatibility: PluginCompatibility::default(),
        },
        source: PluginSource::Local {
            path: "/tmp/test".to_string(),
        },
        installed_at: "2026-03-11T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&plugin).unwrap();
    let parsed: InstalledPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, plugin);
}

#[test]
fn installed_plugin_json_round_trip_with_new_fields() {
    let plugin = InstalledPlugin {
        manifest: PluginManifest {
            name: "my-backend".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A backend plugin".to_string()),
            kind: PluginKind::QualityGate,
            author: Some("Author".to_string()),
            entrypoint: Some("bin/backend".to_string()),
            permissions: vec![PluginPermission::Network, PluginPermission::EnvRead],
            dependencies: vec![PluginDependency {
                name: "dep".to_string(),
                version: Some("^1.0".to_string()),
            }],
            compatibility: PluginCompatibility {
                jig_version: Some("0.2.0".to_string()),
                rust_edition: Some("2024".to_string()),
            },
        },
        source: PluginSource::Git {
            url: "https://github.com/org/plugin.git".to_string(),
            r#ref: Some("v1.0.0".to_string()),
        },
        installed_at: "2026-03-20T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&plugin).unwrap();
    let parsed: InstalledPlugin = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, plugin);
}

// ── PluginError display for ManifestValidation ─────────────

#[test]
fn error_display_manifest_validation() {
    let err = PluginError::ManifestValidation(vec![
        "missing entrypoint".to_string(),
        "bad version".to_string(),
    ]);
    let msg = err.to_string();
    assert!(msg.contains("validation failed"));
    assert!(msg.contains("missing entrypoint"));
    assert!(msg.contains("bad version"));
}
