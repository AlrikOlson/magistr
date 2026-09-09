//! Shareable preset files (`.magistr-preset.toml`) for team onboarding.
//!
//! Encodes a project's magistr configuration in a portable TOML format
//! that can be shared across repos, teams, and machines.
//!
//! Workflow:
//! - `magistr export` → writes `.magistr-preset.toml` from current project config
//! - `magistr import <path>` → scaffolds `.claude/` from a preset file
//! - `magistr init --from <path>` → init using a preset instead of auto-detection

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};
use crate::types::ProjectConfig;

// ── Preset validation ───────────────────────────────────────────────

/// Severity of a preset validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// The preset will not work correctly without fixing this.
    Error,
    /// The preset may produce suboptimal results.
    Warning,
}

/// A single validation issue found in a preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetWarning {
    /// Dotted field path (e.g., "stack.language").
    pub field: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Severity level.
    pub severity: ValidationSeverity,
}

impl PresetWarning {
    fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            severity: ValidationSeverity::Error,
        }
    }

    fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            severity: ValidationSeverity::Warning,
        }
    }

    /// Returns true if this is an error-severity issue.
    pub fn is_error(&self) -> bool {
        self.severity == ValidationSeverity::Error
    }
}

// ── Preset source (local file or URL) ───────────────────────────────

/// Where to load a preset from: a local file path or a remote URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetSource {
    /// Load from a local file.
    Local(PathBuf),
    /// Fetch from a remote URL (http:// or https://).
    Url(String),
}

impl PresetSource {
    /// Parse a string into a `PresetSource`.
    ///
    /// Strings starting with `http://` or `https://` are treated as URLs.
    /// Everything else is treated as a local file path.
    pub fn parse(s: &str) -> Self {
        if s.starts_with("http://") || s.starts_with("https://") {
            Self::Url(s.to_string())
        } else {
            Self::Local(PathBuf::from(s))
        }
    }

    /// Load the preset from this source.
    pub fn load(&self) -> Result<ShareablePreset> {
        match self {
            Self::Local(path) => load_from_file(path),
            Self::Url(url) => fetch_from_url(url),
        }
    }
}

/// Fetch a preset from a remote URL.
fn fetch_from_url(url: &str) -> Result<ShareablePreset> {
    let body: String = ureq::get(url)
        .call()
        .map_err(|e| MagistrError::Config(format!("failed to fetch preset from {url}: {e}")))?
        .body_mut()
        .read_to_string()
        .map_err(|e| {
            MagistrError::Config(format!("failed to read preset response from {url}: {e}"))
        })?;
    ShareablePreset::from_toml(&body)
}

/// Default filename for shareable preset files.
pub const PRESET_FILENAME: &str = ".magistr-preset.toml";

/// A shareable preset that captures a project's magistr configuration.
///
/// All fields needed to scaffold `.claude/` without running detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareablePreset {
    /// Human-readable preset name (e.g., "acme-backend", "my-team-react").
    pub name: String,

    /// Optional description of what this preset is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Stack configuration (language, framework, etc.).
    pub stack: ShareableStack,

    /// Quality gate commands.
    pub quality_gate: ShareableQualityGate,

    /// Formatter configuration.
    pub format: ShareableFormat,

    /// Testing configuration.
    pub testing: ShareableTesting,
}

/// Stack portion of a shareable preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareableStack {
    pub language: String,
    pub framework: String,
    pub package_manager: String,
    pub test_framework: String,
    pub linter: String,
    pub formatter: String,
}

/// Quality gate commands in a shareable preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareableQualityGate {
    pub check: String,
    pub test: String,
    pub lint: String,
}

/// Format configuration in a shareable preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareableFormat {
    pub command: String,
    pub extensions: Vec<String>,
}

/// Testing configuration in a shareable preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareableTesting {
    pub companion_pattern: String,
    pub test_dir: String,
}

// --- From impls: types → shareable (for export) ---

impl From<&crate::types::StackConfig> for ShareableStack {
    fn from(s: &crate::types::StackConfig) -> Self {
        Self {
            language: s.language.clone(),
            framework: s.framework.clone(),
            package_manager: s.package_manager.clone(),
            test_framework: s.test_framework.clone(),
            linter: s.linter.clone(),
            formatter: s.formatter.clone(),
        }
    }
}

impl From<&crate::types::QualityGate> for ShareableQualityGate {
    fn from(qg: &crate::types::QualityGate) -> Self {
        Self {
            check: qg.check.clone(),
            test: qg.test.clone(),
            lint: qg.lint.clone(),
        }
    }
}

impl From<&crate::types::FormatConfig> for ShareableFormat {
    fn from(f: &crate::types::FormatConfig) -> Self {
        Self {
            command: f.command.clone(),
            extensions: f.extensions.clone(),
        }
    }
}

impl From<&crate::types::TestingConfig> for ShareableTesting {
    fn from(t: &crate::types::TestingConfig) -> Self {
        Self {
            companion_pattern: t.companion_pattern.clone(),
            test_dir: t.test_dir.clone(),
        }
    }
}

// --- From impls: shareable → types (for import) ---

impl From<&ShareableQualityGate> for crate::types::QualityGate {
    fn from(sqg: &ShareableQualityGate) -> Self {
        Self {
            check: sqg.check.clone(),
            test: sqg.test.clone(),
            lint: sqg.lint.clone(),
        }
    }
}

impl From<&ShareableFormat> for crate::types::FormatConfig {
    fn from(sf: &ShareableFormat) -> Self {
        Self {
            command: sf.command.clone(),
            extensions: sf.extensions.clone(),
            exclusions: Vec::new(),
        }
    }
}

impl From<&ShareableTesting> for crate::types::TestingConfig {
    fn from(st: &ShareableTesting) -> Self {
        Self {
            companion_pattern: st.companion_pattern.clone(),
            test_dir: st.test_dir.clone(),
        }
    }
}

impl ShareablePreset {
    /// Create a preset from an existing `ProjectConfig` (e.g., from `.claude/project.json`).
    pub fn from_project_config(config: &ProjectConfig) -> Self {
        Self {
            name: config.project.name.clone(),
            description: if config.project.description.is_empty() {
                None
            } else {
                Some(config.project.description.clone())
            },
            stack: ShareableStack::from(&config.stack),
            quality_gate: ShareableQualityGate::from(&config.quality_gate),
            format: ShareableFormat::from(&config.format),
            testing: ShareableTesting::from(&config.testing),
        }
    }

    /// Convert this preset into template substitution variables for scaffolding.
    ///
    /// The returned map has the same keys that `scaffold::generate()` expects.
    pub fn to_template_vars(&self, project_name: &str) -> HashMap<String, String> {
        let format_extensions = self
            .format
            .extensions
            .iter()
            .map(|e| format!("\"{e}\""))
            .collect::<Vec<_>>()
            .join(", ");

        let mut vars = HashMap::new();
        vars.insert("project_name".into(), project_name.into());
        vars.insert(
            "project_description".into(),
            self.description.clone().unwrap_or_default(),
        );
        vars.insert("project_type".into(), String::new());
        vars.insert("language".into(), self.stack.language.clone());
        vars.insert("framework".into(), self.stack.framework.clone());
        vars.insert("package_manager".into(), self.stack.package_manager.clone());
        vars.insert("test_framework".into(), self.stack.test_framework.clone());
        vars.insert("linter".into(), self.stack.linter.clone());
        vars.insert("formatter".into(), self.stack.formatter.clone());
        vars.insert("check_cmd".into(), self.quality_gate.check.clone());
        vars.insert("test_cmd".into(), self.quality_gate.test.clone());
        vars.insert("lint_cmd".into(), self.quality_gate.lint.clone());
        vars.insert("format_cmd".into(), self.format.command.clone());
        vars.insert("format_extensions".into(), format_extensions);
        vars.insert(
            "companion_pattern".into(),
            self.testing.companion_pattern.clone(),
        );
        vars.insert("test_dir".into(), self.testing.test_dir.clone());
        vars.insert("state_prefix".into(), sanitize_prefix(project_name));
        vars.insert("allowed_tools".into(), String::new());
        vars
    }

    /// Validate this preset and return any issues found.
    ///
    /// Returns an empty vec if the preset is valid.
    pub fn validate(&self) -> Vec<PresetWarning> {
        let mut warnings = Vec::new();

        if self.name.trim().is_empty() {
            warnings.push(PresetWarning::error("name", "preset name is required"));
        }

        if self.stack.language.trim().is_empty() {
            warnings.push(PresetWarning::error(
                "stack.language",
                "language is required",
            ));
        }

        if self.quality_gate.check.trim().is_empty() {
            warnings.push(PresetWarning::warning(
                "quality_gate.check",
                "no check command — quality gate will skip type checking",
            ));
        }

        if self.quality_gate.test.trim().is_empty() {
            warnings.push(PresetWarning::warning(
                "quality_gate.test",
                "no test command — quality gate will skip tests",
            ));
        }

        if self.quality_gate.lint.trim().is_empty() {
            warnings.push(PresetWarning::warning(
                "quality_gate.lint",
                "no lint command — quality gate will skip linting",
            ));
        }

        if self.format.command.trim().is_empty() {
            warnings.push(PresetWarning::warning(
                "format.command",
                "no format command — auto-formatting disabled",
            ));
        }

        if self.format.extensions.is_empty() {
            warnings.push(PresetWarning::warning(
                "format.extensions",
                "no file extensions — formatter won't know which files to process",
            ));
        }

        warnings
    }

    /// Returns true if validation found any error-severity issues.
    pub fn has_validation_errors(&self) -> bool {
        self.validate().iter().any(PresetWarning::is_error)
    }

    /// Serialize this preset to TOML.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| MagistrError::Config(format!("TOML serialize: {e}")))
    }

    /// Deserialize a preset from TOML.
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| MagistrError::Config(format!("TOML parse: {e}")))
    }
}

/// Save a preset to a file.
pub fn save_to_file(path: &Path, preset: &ShareablePreset) -> Result<()> {
    let content = preset.to_toml()?;
    std::fs::write(path, content).map_err(|e| MagistrError::io(path, e))
}

/// Load a preset from a file.
pub fn load_from_file(path: &Path) -> Result<ShareablePreset> {
    let content = std::fs::read_to_string(path).map_err(|e| MagistrError::io(path, e))?;
    ShareablePreset::from_toml(&content)
}

/// Export a preset from a project directory by reading `.magistr/project.json`.
pub fn export_preset(dir: &Path) -> Result<ShareablePreset> {
    let project_json = crate::paths::project_json(dir);
    if !project_json.exists() {
        return Err(MagistrError::Config(
            "no .magistr/project.json found — run `magistr init` first".into(),
        ));
    }
    let content =
        std::fs::read_to_string(&project_json).map_err(|e| MagistrError::io(&project_json, e))?;
    let config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| MagistrError::Config(format!("JSON parse: {e}")))?;
    Ok(ShareablePreset::from_project_config(&config))
}

/// Sanitize a project name into a slug suitable for state prefix.
///
/// Converts to lowercase, replaces non-ASCII-alphanumeric characters with dashes,
/// collapses consecutive dashes, and trims leading/trailing dashes.
fn sanitize_prefix(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
#[path = "shareable.tests.rs"]
mod tests;
