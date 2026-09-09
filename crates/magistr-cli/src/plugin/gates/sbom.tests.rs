use super::*;

#[test]
fn sbom_format_extension() {
    assert_eq!(SbomFormat::CycloneDxJson.extension(), "cdx.json");
    assert_eq!(SbomFormat::CycloneDxXml.extension(), "cdx.xml");
    assert_eq!(SbomFormat::SpdxJson.extension(), "spdx.json");
}

#[test]
fn sbom_format_label() {
    assert_eq!(SbomFormat::CycloneDxJson.label(), "cyclonedx-json");
    assert_eq!(SbomFormat::CycloneDxXml.label(), "cyclonedx-xml");
    assert_eq!(SbomFormat::SpdxJson.label(), "spdx-json");
}

#[test]
fn sbom_generator_command() {
    assert_eq!(SbomGenerator::CargoSbom.command(), "cargo-sbom");
    assert_eq!(SbomGenerator::Syft.command(), "syft");
    assert_eq!(SbomGenerator::CycloneDxCli.command(), "cyclonedx-cli");
    assert_eq!(SbomGenerator::Auto.command(), "auto");
}

#[test]
fn sbom_config_default() {
    let cfg = SbomConfig::default();
    assert_eq!(cfg.generator, SbomGenerator::Auto);
    assert_eq!(cfg.format, SbomFormat::CycloneDxJson);
    assert!(cfg.min_components.is_none());
    assert!(!cfg.require_tool);
}

#[test]
fn sbom_config_from_json() {
    let json = serde_json::json!({
        "generator": "syft",
        "format": "spdx-json",
        "min_components": 10,
        "require_tool": true,
    });

    let cfg = SbomConfig::from_json(&json);
    assert_eq!(cfg.generator, SbomGenerator::Syft);
    assert_eq!(cfg.format, SbomFormat::SpdxJson);
    assert_eq!(cfg.min_components, Some(10));
    assert!(cfg.require_tool);
}

#[test]
fn sbom_config_from_json_defaults() {
    let json = serde_json::json!({});
    let cfg = SbomConfig::from_json(&json);
    assert_eq!(cfg.generator, SbomGenerator::Auto);
    assert_eq!(cfg.format, SbomFormat::CycloneDxJson);
}

#[test]
fn sbom_gate_name() {
    let gate = SbomGate::new();
    assert_eq!(gate.name(), "sbom");
}

#[test]
fn sbom_gate_names() {
    let names = SbomGate::gate_names();
    assert_eq!(names, vec!["sbom"]);
}

#[test]
fn validate_sbom_cyclonedx_json() {
    let dir = tempfile::TempDir::new().unwrap();
    let sbom = serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "components": [
            {
                "type": "library",
                "name": "serde",
                "version": "1.0.200",
                "licenses": [{"license": {"id": "MIT"}}]
            },
            {
                "type": "library",
                "name": "tokio",
                "version": "1.38.0",
                "licenses": [{"license": {"id": "MIT"}}]
            },
            {
                "type": "library",
                "name": "mystery-crate",
                "version": "0.1.0"
            }
        ]
    });

    let sbom_path = dir.path().join("sbom.cdx.json");
    std::fs::write(&sbom_path, serde_json::to_string_pretty(&sbom).unwrap()).unwrap();

    let config = SbomConfig::default();
    let (passed, findings, summary) = SbomGate::validate_sbom(&sbom_path, &config);

    assert!(passed);
    // Should have a warning about unlicensed component
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id.as_deref() == Some("SBOM-UNLICENSED")),
        "expected SBOM-UNLICENSED finding"
    );
    assert!(summary.contains("3 components"));
}

#[test]
fn validate_sbom_min_components_fail() {
    let dir = tempfile::TempDir::new().unwrap();
    let sbom = serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "components": [
            {"type": "library", "name": "serde", "version": "1.0", "licenses": []}
        ]
    });

    let sbom_path = dir.path().join("sbom.cdx.json");
    std::fs::write(&sbom_path, serde_json::to_string_pretty(&sbom).unwrap()).unwrap();

    let config = SbomConfig {
        min_components: Some(5),
        ..Default::default()
    };
    let (passed, findings, _) = SbomGate::validate_sbom(&sbom_path, &config);

    assert!(!passed);
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id.as_deref() == Some("SBOM-MIN-COMPONENTS"))
    );
}

#[test]
fn validate_sbom_spdx_format() {
    let dir = tempfile::TempDir::new().unwrap();
    let sbom = serde_json::json!({
        "spdxVersion": "SPDX-2.3",
        "packages": [
            {"name": "serde", "versionInfo": "1.0"},
            {"name": "tokio", "versionInfo": "1.38"}
        ]
    });

    let sbom_path = dir.path().join("sbom.spdx.json");
    std::fs::write(&sbom_path, serde_json::to_string_pretty(&sbom).unwrap()).unwrap();

    let config = SbomConfig::default();
    let (passed, _, summary) = SbomGate::validate_sbom(&sbom_path, &config);

    assert!(passed);
    assert!(summary.contains("2 components"));
}

#[test]
fn validate_sbom_unreadable() {
    let config = SbomConfig::default();
    let (passed, findings, _) =
        SbomGate::validate_sbom(std::path::Path::new("/nonexistent/sbom.json"), &config);

    assert!(!passed);
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id.as_deref() == Some("SBOM-READ"))
    );
}

#[test]
fn validate_sbom_invalid_json() {
    let dir = tempfile::TempDir::new().unwrap();
    let sbom_path = dir.path().join("sbom.cdx.json");
    std::fs::write(&sbom_path, "not valid json {{{").unwrap();

    let config = SbomConfig::default();
    let (passed, findings, _) = SbomGate::validate_sbom(&sbom_path, &config);

    assert!(!passed);
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id.as_deref() == Some("SBOM-PARSE"))
    );
}

#[test]
fn validate_sbom_xml_format() {
    let dir = tempfile::TempDir::new().unwrap();
    let sbom_path = dir.path().join("sbom.cdx.xml");
    std::fs::write(
        &sbom_path,
        r#"<?xml version="1.0"?><bom xmlns="http://cyclonedx.org/schema/bom/1.5"></bom>"#,
    )
    .unwrap();

    let config = SbomConfig::default();
    let (passed, findings, _) = SbomGate::validate_sbom(&sbom_path, &config);

    assert!(passed);
    assert!(
        findings
            .iter()
            .any(|f| f.rule_id.as_deref() == Some("SBOM-FORMAT"))
    );
}

#[tokio::test]
async fn sbom_gate_no_tool_available() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut gate = SbomGate::with_config(SbomConfig {
        generator: SbomGenerator::Auto,
        require_tool: false,
        ..Default::default()
    });

    let config = GateConfig {
        gate_name: "sbom".to_string(),
        project_dir: dir.path().display().to_string(),
        config: None,
    };

    let outcome = gate.run_gate(config).await.unwrap();
    // If no SBOM tools are installed, it should pass with a warning (require_tool=false)
    // or find a tool and run — either way it shouldn't error
    assert!(
        outcome.passed
            || outcome
                .findings
                .iter()
                .any(|f| f.rule_id.as_deref() == Some("SBOM-TOOL-MISSING")
                    || f.rule_id.as_deref() == Some("SBOM-GENERATE"))
    );
}
