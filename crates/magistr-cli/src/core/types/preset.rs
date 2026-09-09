use serde::{Deserialize, Serialize};

use super::{FormatConfig, QualityGate, TestingConfig};

/// A language preset loaded from embedded JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    pub quality_gate: QualityGate,
    pub format: FormatConfig,
    pub testing: TestingConfig,
    pub allowed_tools: Vec<String>,
}

/// A framework-specific preset overlay. All fields are optional —
/// only set fields override the base language preset.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PresetOverride {
    #[serde(default)]
    pub quality_gate: Option<QualityGateOverride>,
    #[serde(default)]
    pub format: Option<FormatConfigOverride>,
    #[serde(default)]
    pub testing: Option<TestingConfigOverride>,
    #[serde(default)]
    pub extra_allowed_tools: Vec<String>,
}

/// Partial quality gate — only overrides fields that are Some.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct QualityGateOverride {
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default)]
    pub test: Option<String>,
    #[serde(default)]
    pub lint: Option<String>,
}

/// Partial format config — only overrides fields that are Some.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FormatConfigOverride {
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub exclusions: Option<Vec<String>>,
}

/// Partial testing config — only overrides fields that are Some.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TestingConfigOverride {
    #[serde(default)]
    pub companion_pattern: Option<String>,
    #[serde(default)]
    pub test_dir: Option<String>,
}

impl Preset {
    /// Merge a framework overlay onto this base preset.
    /// Overlay fields that are Some replace the base; None fields keep the base.
    pub fn merge(mut self, overlay: &PresetOverride) -> Self {
        if let Some(qg) = &overlay.quality_gate {
            if let Some(check) = &qg.check {
                self.quality_gate.check = check.clone();
            }
            if let Some(test) = &qg.test {
                self.quality_gate.test = test.clone();
            }
            if let Some(lint) = &qg.lint {
                self.quality_gate.lint = lint.clone();
            }
        }
        if let Some(fmt) = &overlay.format {
            if let Some(extensions) = &fmt.extensions {
                self.format.extensions = extensions.clone();
            }
            if let Some(command) = &fmt.command {
                self.format.command = command.clone();
            }
            if let Some(exclusions) = &fmt.exclusions {
                self.format.exclusions = exclusions.clone();
            }
        }
        if let Some(testing) = &overlay.testing {
            if let Some(pattern) = &testing.companion_pattern {
                self.testing.companion_pattern = pattern.clone();
            }
            if let Some(test_dir) = &testing.test_dir {
                self.testing.test_dir = test_dir.clone();
            }
        }
        if !overlay.extra_allowed_tools.is_empty() {
            self.allowed_tools
                .extend(overlay.extra_allowed_tools.clone());
        }
        self
    }
}
