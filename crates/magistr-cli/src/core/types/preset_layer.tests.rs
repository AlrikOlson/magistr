use super::*;

use crate::types::{
    FormatConfig, FormatConfigOverride, Preset, PresetOverride, QualityGate, QualityGateOverride,
    TestingConfig,
};

fn base_preset() -> Preset {
    Preset {
        quality_gate: QualityGate {
            check: "cargo check".to_string(),
            test: "cargo test".to_string(),
            lint: "cargo clippy".to_string(),
        },
        format: FormatConfig {
            extensions: vec!["rs".to_string()],
            command: "rustfmt".to_string(),
            exclusions: vec![],
        },
        testing: TestingConfig {
            companion_pattern: "{name}.tests.rs".to_string(),
            test_dir: "alongside".to_string(),
        },
        allowed_tools: vec!["Read".to_string(), "Write".to_string()],
    }
}

fn make_layer(layer_type: PresetLayerType, name: &str, check_cmd: &str) -> PresetLayer {
    PresetLayer {
        layer_type,
        name: name.to_string(),
        overrides: PresetOverride {
            quality_gate: Some(QualityGateOverride {
                check: Some(check_cmd.to_string()),
                test: None,
                lint: None,
            }),
            format: None,
            testing: None,
            extra_allowed_tools: vec![],
        },
    }
}

#[test]
fn empty_pipeline_returns_base_unchanged() {
    let base = base_preset();
    let pipeline = PresetPipeline::new();
    let result = pipeline.apply(base.clone());
    assert_eq!(result, base);
}

#[test]
fn single_layer_applies_override() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();
    pipeline.add(make_layer(
        PresetLayerType::Framework,
        "axum",
        "cargo check --all-targets",
    ));
    let result = pipeline.apply(base);
    assert_eq!(result.quality_gate.check, "cargo check --all-targets");
    // Unoverridden fields preserved
    assert_eq!(result.quality_gate.test, "cargo test");
}

#[test]
fn layers_applied_in_type_order() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    // Add in reverse order — pipeline should still apply in type order
    pipeline.add(make_layer(
        PresetLayerType::UserOverride,
        "user",
        "user-check",
    ));
    pipeline.add(make_layer(
        PresetLayerType::Framework,
        "framework",
        "framework-check",
    ));
    pipeline.add(make_layer(
        PresetLayerType::Database,
        "database",
        "database-check",
    ));

    let result = pipeline.apply(base);
    // UserOverride is last, so it wins
    assert_eq!(result.quality_gate.check, "user-check");
}

#[test]
fn later_layer_type_overrides_earlier() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    pipeline.add(make_layer(
        PresetLayerType::Framework,
        "framework",
        "framework-check",
    ));
    pipeline.add(make_layer(
        PresetLayerType::Deployment,
        "docker",
        "docker-check",
    ));

    let result = pipeline.apply(base);
    assert_eq!(result.quality_gate.check, "docker-check");
}

#[test]
fn user_override_always_wins() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    pipeline.add(make_layer(
        PresetLayerType::Framework,
        "framework",
        "framework-check",
    ));
    pipeline.add(make_layer(PresetLayerType::Database, "db", "db-check"));
    pipeline.add(make_layer(PresetLayerType::CiCd, "ci", "ci-check"));
    pipeline.add(make_layer(
        PresetLayerType::Deployment,
        "deploy",
        "deploy-check",
    ));
    pipeline.add(make_layer(
        PresetLayerType::ProjectLocal,
        "project",
        "project-check",
    ));
    pipeline.add(make_layer(
        PresetLayerType::UserOverride,
        "user",
        "user-check",
    ));

    let result = pipeline.apply(base);
    assert_eq!(result.quality_gate.check, "user-check");
}

#[test]
fn missing_layers_graceful_fallback() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    // Only add database layer, skip framework/CI/deployment/etc.
    pipeline.add(make_layer(
        PresetLayerType::Database,
        "prisma",
        "prisma-check",
    ));

    let result = pipeline.apply(base);
    assert_eq!(result.quality_gate.check, "prisma-check");
    // Other fields preserved from base
    assert_eq!(result.quality_gate.test, "cargo test");
    assert_eq!(result.format.command, "rustfmt");
}

#[test]
fn add_optional_skips_none() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();
    pipeline.add_optional(None);
    pipeline.add_optional(Some(make_layer(
        PresetLayerType::Database,
        "diesel",
        "diesel-check",
    )));
    assert_eq!(pipeline.len(), 1);
    let result = pipeline.apply(base);
    assert_eq!(result.quality_gate.check, "diesel-check");
}

#[test]
fn extra_allowed_tools_accumulate() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    pipeline.add(PresetLayer {
        layer_type: PresetLayerType::Database,
        name: "sqlx".to_string(),
        overrides: PresetOverride {
            quality_gate: None,
            format: None,
            testing: None,
            extra_allowed_tools: vec!["Bash(sqlx:*)".to_string()],
        },
    });
    pipeline.add(PresetLayer {
        layer_type: PresetLayerType::CiCd,
        name: "github-actions".to_string(),
        overrides: PresetOverride {
            quality_gate: None,
            format: None,
            testing: None,
            extra_allowed_tools: vec!["Bash(gh:*)".to_string()],
        },
    });

    let result = pipeline.apply(base);
    assert!(result.allowed_tools.contains(&"Read".to_string()));
    assert!(result.allowed_tools.contains(&"Bash(sqlx:*)".to_string()));
    assert!(result.allowed_tools.contains(&"Bash(gh:*)".to_string()));
}

#[test]
fn same_type_layers_preserve_insertion_order() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    // Two project-local layers — second should win (applied last)
    pipeline.add(make_layer(
        PresetLayerType::ProjectLocal,
        "local-a",
        "check-a",
    ));
    pipeline.add(make_layer(
        PresetLayerType::ProjectLocal,
        "local-b",
        "check-b",
    ));

    let result = pipeline.apply(base);
    assert_eq!(result.quality_gate.check, "check-b");
}

#[test]
fn pipeline_len_and_is_empty() {
    let mut pipeline = PresetPipeline::new();
    assert!(pipeline.is_empty());
    assert_eq!(pipeline.len(), 0);

    pipeline.add(make_layer(PresetLayerType::Database, "db", "check"));
    assert!(!pipeline.is_empty());
    assert_eq!(pipeline.len(), 1);
}

#[test]
fn layer_type_ordering() {
    assert!(PresetLayerType::Language < PresetLayerType::Framework);
    assert!(PresetLayerType::Framework < PresetLayerType::PackageManager);
    assert!(PresetLayerType::PackageManager < PresetLayerType::Database);
    assert!(PresetLayerType::Database < PresetLayerType::CiCd);
    assert!(PresetLayerType::CiCd < PresetLayerType::Deployment);
    assert!(PresetLayerType::Deployment < PresetLayerType::ProjectLocal);
    assert!(PresetLayerType::ProjectLocal < PresetLayerType::UserOverride);
}

#[test]
fn layer_type_display() {
    assert_eq!(PresetLayerType::Database.to_string(), "database");
    assert_eq!(PresetLayerType::CiCd.to_string(), "ci-cd");
    assert_eq!(PresetLayerType::ProjectLocal.to_string(), "project-local");
}

#[test]
fn format_override_in_pipeline() {
    let base = base_preset();
    let mut pipeline = PresetPipeline::new();

    pipeline.add(PresetLayer {
        layer_type: PresetLayerType::Deployment,
        name: "docker".to_string(),
        overrides: PresetOverride {
            quality_gate: None,
            format: Some(FormatConfigOverride {
                extensions: None,
                command: None,
                exclusions: Some(vec!["Dockerfile".to_string()]),
            }),
            testing: None,
            extra_allowed_tools: vec![],
        },
    });

    let result = pipeline.apply(base);
    assert!(result.format.exclusions.contains(&"Dockerfile".to_string()));
    // Command preserved from base
    assert_eq!(result.format.command, "rustfmt");
}
