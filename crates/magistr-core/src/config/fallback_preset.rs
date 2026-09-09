//! Fallback preset for unknown languages.

use crate::types::Preset;

/// Get a fallback preset for unknown languages.
pub fn fallback_preset() -> Preset {
    Preset {
        quality_gate: crate::types::QualityGate {
            check: "echo 'no check configured'".to_string(),
            test: "echo 'no test configured'".to_string(),
            lint: "echo 'no lint configured'".to_string(),
        },
        format: crate::types::FormatConfig {
            extensions: vec![],
            command: String::new(),
            exclusions: vec![],
        },
        testing: crate::types::TestingConfig {
            companion_pattern: "{name}.test.{ext}".to_string(),
            test_dir: "tests/".to_string(),
        },
        allowed_tools: vec![
            "Read".to_string(),
            "Write".to_string(),
            "Edit".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
            "Bash(git:*)".to_string(),
        ],
    }
}
