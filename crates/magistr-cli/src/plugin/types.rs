//! Plugin types — manifest, kind, and metadata.
//!
//! A plugin is a directory containing a `magistr-plugin.toml` manifest
//! and associated resources. Two categories exist:
//!
//! - **Declarative plugins** — detection rules and scaffold templates
//!   (no executable code, just config + templates)
//! - **Executable plugins** — workflow steps and quality gates that run as
//!   subprocesses with declared permissions
//!
//! No kind reaches a model: that is ACP's job (`magistr acp-agent`), and the
//! retired `backend` kind is refused by name.

use serde::{Deserialize, Serialize};

/// The manifest filename expected in every plugin directory.
pub const MANIFEST_FILENAME: &str = "magistr-plugin.toml";

/// The kind the plugin bridge once reached a model with. Retired: every
/// model is reached over ACP, so a manifest declaring it is refused with
/// [`BACKEND_KIND_RETIRED`] rather than the parser's generic unknown-variant.
const RETIRED_BACKEND_KIND: &str = "backend";

/// What a manifest still declaring the retired `backend` kind is told.
const BACKEND_KIND_RETIRED: &str = "plugin kind \"backend\" was retired: a model is reached over ACP only — \
     `magistr acp-agent --provider claude|codex|openai|gemini|ollama` serves the providers and any native \
     ACP agent on PATH is spoken to as-is; quality-gate and workflow-step plugins are unaffected";

/// What kind of functionality a plugin provides.
///
/// Plugins are tools, never agents — there is no kind that reaches a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginKind {
    /// Custom stack detection rules (language, framework, tools).
    Detection,
    /// Custom scaffold templates (rules, hooks, skills).
    Template,
    /// Custom workflow step (pre/post hooks in the iteration loop).
    WorkflowStep,
    /// Custom quality gate (e.g., SonarQube, license compliance).
    QualityGate,
}

impl PluginKind {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Detection => "detection",
            Self::Template => "template",
            Self::WorkflowStep => "workflow-step",
            Self::QualityGate => "quality-gate",
        }
    }

    /// Whether this plugin kind requires an entrypoint (executable plugins).
    #[must_use]
    pub fn is_executable(self) -> bool {
        matches!(self, Self::WorkflowStep | Self::QualityGate)
    }
}

impl std::fmt::Display for PluginKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Where a plugin was installed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum PluginSource {
    /// Installed from a local directory path.
    Local { path: String },
    /// Installed from a git repository URL.
    Git { url: String, r#ref: Option<String> },
    /// Installed from the central plugin registry by name.
    Registry { name: String, version: String },
}

impl std::fmt::Display for PluginSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local { path } => write!(f, "local:{path}"),
            Self::Git { url, r#ref } => {
                if let Some(git_ref) = r#ref {
                    write!(f, "git:{url}@{git_ref}")
                } else {
                    write!(f, "git:{url}")
                }
            }
            Self::Registry { name, version } => write!(f, "registry:{name}@{version}"),
        }
    }
}

/// A permission that a plugin declares it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginPermission {
    /// Read files from the project directory.
    FileRead,
    /// Write files to the project directory.
    FileWrite,
    /// Make outbound network requests.
    Network,
    /// Read environment variables.
    EnvRead,
    /// Execute shell commands.
    ShellExec,
}

impl PluginPermission {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::FileRead => "file-read",
            Self::FileWrite => "file-write",
            Self::Network => "network",
            Self::EnvRead => "env-read",
            Self::ShellExec => "shell-exec",
        }
    }
}

impl std::fmt::Display for PluginPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A dependency on another plugin, with an optional semver version requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Name of the required plugin.
    pub name: String,
    /// Semver version requirement (e.g., ">=1.0.0", "^2.0").
    /// If absent, any version is accepted.
    #[serde(default)]
    pub version: Option<String>,
}

/// Compatibility constraints for a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginCompatibility {
    /// Minimum magistr version required (semver, e.g., "0.2.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jig_version: Option<String>,
    /// Rust edition required (e.g., "2024"). Only relevant for cdylib plugins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_edition: Option<String>,
}

/// Plugin manifest — parsed from `magistr-plugin.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin name (e.g., "my-company-stack").
    pub name: String,
    /// Plugin version (semver string).
    pub version: String,
    /// Short description of what the plugin provides.
    #[serde(default)]
    pub description: Option<String>,
    /// What kind of plugin this is.
    pub kind: PluginKind,
    /// Author name or organization.
    #[serde(default)]
    pub author: Option<String>,
    /// Path to the plugin entrypoint (binary or cdylib), relative to plugin dir.
    /// Required for executable plugin kinds (workflow-step, quality-gate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Permissions this plugin requires.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PluginPermission>,
    /// Other plugins this plugin depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PluginDependency>,
    /// Compatibility constraints (magistr version, rust edition).
    #[serde(default, skip_serializing_if = "is_default_compat")]
    pub compatibility: PluginCompatibility,
}

fn is_default_compat(c: &PluginCompatibility) -> bool {
    c.jig_version.is_none() && c.rust_edition.is_none()
}

impl PluginManifest {
    /// Parse a manifest from TOML content.
    ///
    /// A manifest declaring the retired `backend` kind is refused with a
    /// pointer at the one bridge a model is reached over now, so a plugin
    /// left over from the bridge era gets told what replaced it rather
    /// than an unknown-variant.
    pub fn from_toml(content: &str) -> Result<Self, PluginError> {
        let value: toml::Value =
            toml::from_str(content).map_err(|e| PluginError::ManifestParse(e.to_string()))?;
        if value.get("kind").and_then(toml::Value::as_str) == Some(RETIRED_BACKEND_KIND) {
            return Err(PluginError::ManifestParse(BACKEND_KIND_RETIRED.to_owned()));
        }
        value
            .try_into()
            .map_err(|e: toml::de::Error| PluginError::ManifestParse(e.to_string()))
    }

    /// Serialize the manifest to TOML.
    pub fn to_toml(&self) -> Result<String, PluginError> {
        toml::to_string_pretty(self).map_err(|e| PluginError::ManifestSerialize(e.to_string()))
    }

    /// Validate manifest consistency.
    ///
    /// Returns a list of validation errors. An empty list means the manifest is valid.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Executable plugins must have an entrypoint
        if self.kind.is_executable() && self.entrypoint.is_none() {
            errors.push(format!(
                "{} plugin '{}' requires an entrypoint",
                self.kind.label(),
                self.name,
            ));
        }

        // Declarative plugins should not have an entrypoint
        if !self.kind.is_executable() && self.entrypoint.is_some() {
            errors.push(format!(
                "{} plugin '{}' should not have an entrypoint",
                self.kind.label(),
                self.name,
            ));
        }

        errors
    }
}

/// A detection rule within a detection plugin.
///
/// Matches projects based on file patterns and/or content patterns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionRule {
    /// What this rule detects (e.g., "internal-framework-x").
    pub name: String,
    /// Files whose presence triggers this rule (glob patterns).
    #[serde(default)]
    pub file_patterns: Vec<String>,
    /// Content patterns to search for in matched files (plain text).
    #[serde(default)]
    pub content_patterns: Vec<String>,
    /// Confidence score if matched (0–100).
    #[serde(default = "default_confidence")]
    pub confidence: u8,
    /// Category: "framework", "language", "tool", etc.
    #[serde(default = "default_category")]
    pub category: String,
}

fn default_confidence() -> u8 {
    80
}

fn default_category() -> String {
    "framework".to_string()
}

/// Detection plugin configuration — loaded from `detect.toml` in the plugin dir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionConfig {
    /// Detection rules provided by this plugin.
    #[serde(default)]
    pub rules: Vec<DetectionRule>,
}

impl DetectionConfig {
    /// Parse from TOML content.
    pub fn from_toml(content: &str) -> Result<Self, PluginError> {
        toml::from_str(content).map_err(|e| PluginError::ManifestParse(e.to_string()))
    }
}

/// Template plugin configuration — loaded from `templates.toml` in the plugin dir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateConfig {
    /// Files to copy into the scaffold output, relative to the plugin dir.
    #[serde(default)]
    pub files: Vec<TemplateFile>,
}

impl TemplateConfig {
    /// Parse from TOML content.
    pub fn from_toml(content: &str) -> Result<Self, PluginError> {
        toml::from_str(content).map_err(|e| PluginError::ManifestParse(e.to_string()))
    }
}

/// A template file entry — source path in plugin dir and target path in scaffold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateFile {
    /// Source path relative to the plugin directory.
    pub src: String,
    /// Target path relative to `.claude/` in the project.
    pub dest: String,
}

/// An installed plugin with its manifest and source info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledPlugin {
    /// The parsed manifest.
    pub manifest: PluginManifest,
    /// Where it was installed from.
    pub source: PluginSource,
    /// When it was installed (ISO 8601 string).
    pub installed_at: String,
}

// ── Plugin Protocol Messages ────────────────────────────────────────────────

/// A request sent from magistr to an executable plugin over stdin.
///
/// Each request carries a monotonic `id` for response correlation.
/// Serialized as a single JSON line (`\n`-delimited).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginRequest {
    /// Initialize the plugin with host metadata.
    Init {
        id: u64,
        jig_version: String,
        plugin_dir: String,
    },
    /// Activate the plugin for a specific project.
    Activate { id: u64, project_dir: String },
    /// Deactivate the plugin (graceful shutdown).
    Deactivate { id: u64 },
    /// Run a quality gate check.
    #[serde(rename = "gate.run")]
    GateRun {
        id: u64,
        /// Gate name to run (e.g., "sonarqube", "license").
        gate_name: String,
        /// Project directory to check.
        project_dir: String,
        /// Optional gate-specific configuration as JSON.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        config: Option<serde_json::Value>,
    },
    /// Run a workflow step.
    #[serde(rename = "step.run")]
    WorkflowStepRun {
        id: u64,
        /// Step name to run (e.g., "security-scan", "code-review").
        step_name: String,
        /// Workflow phase: "pre_research", "post_research", "pre_execute",
        /// "post_execute", "pre_verify", "post_verify".
        phase: String,
        /// Project directory.
        project_dir: String,
        /// Optional step-specific configuration as JSON.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        config: Option<serde_json::Value>,
    },
    /// Store a session record.
    #[serde(rename = "storage.store")]
    StorageStore {
        id: u64,
        /// Session record serialized as JSON.
        record: serde_json::Value,
    },
    /// Retrieve a session record by session ID.
    #[serde(rename = "storage.get")]
    StorageGet {
        id: u64,
        /// Session identifier.
        session_id: String,
    },
    /// Query session records with a filter.
    #[serde(rename = "storage.query")]
    StorageQuery {
        id: u64,
        /// Filter criteria as JSON.
        filter: serde_json::Value,
    },
    /// Delete a session record.
    #[serde(rename = "storage.delete")]
    StorageDelete {
        id: u64,
        /// Session identifier.
        session_id: String,
    },
}

impl PluginRequest {
    /// Get the request ID for response correlation.
    #[must_use]
    pub fn id(&self) -> u64 {
        match self {
            Self::Init { id, .. }
            | Self::Activate { id, .. }
            | Self::Deactivate { id }
            | Self::GateRun { id, .. }
            | Self::WorkflowStepRun { id, .. }
            | Self::StorageStore { id, .. }
            | Self::StorageGet { id, .. }
            | Self::StorageQuery { id, .. }
            | Self::StorageDelete { id, .. } => *id,
        }
    }
}

/// A response sent from an executable plugin to magistr over stdout.
///
/// Each response carries the `id` of the request it answers.
/// Serialized as a single JSON line (`\n`-delimited).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginResponse {
    /// Plugin initialized successfully.
    InitOk { id: u64, capabilities: Vec<String> },
    /// Plugin activated for a project.
    ActivateOk { id: u64 },
    /// Plugin deactivated.
    DeactivateOk { id: u64 },
    /// A single finding from a quality gate (streamed incrementally).
    #[serde(rename = "gate.finding")]
    GateFinding {
        id: u64,
        /// File path where the finding was detected (if applicable).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        /// Line number in the file (if applicable).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        /// Human-readable finding message.
        message: String,
        /// Severity: "error", "warning", or "info".
        severity: String,
        /// Rule identifier (e.g., "S1234" for SonarQube).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule_id: Option<String>,
    },
    /// Quality gate completed.
    #[serde(rename = "gate.done")]
    GateDone {
        id: u64,
        /// Whether the gate passed overall.
        passed: bool,
        /// Human-readable summary.
        summary: String,
    },
    /// Progress update from a workflow step (streamed incrementally).
    #[serde(rename = "step.progress")]
    WorkflowStepProgress {
        id: u64,
        /// Human-readable progress message.
        message: String,
        /// Optional progress fraction (0.0–1.0).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        progress: Option<f32>,
    },
    /// A single finding from a workflow step (streamed incrementally).
    #[serde(rename = "step.finding")]
    WorkflowStepFinding {
        id: u64,
        /// Human-readable finding message.
        message: String,
        /// Severity: "error", "warning", or "info".
        severity: String,
        /// File path where the finding was detected (if applicable).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        /// Line number in the file (if applicable).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
    },
    /// Workflow step completed.
    #[serde(rename = "step.done")]
    WorkflowStepDone {
        id: u64,
        /// Whether the step passed overall.
        passed: bool,
        /// Human-readable summary.
        summary: String,
    },
    /// Storage operation completed successfully with a record.
    #[serde(rename = "storage.record")]
    StorageRecord {
        id: u64,
        /// The session record as JSON.
        record: serde_json::Value,
    },
    /// Storage query completed with results.
    #[serde(rename = "storage.records")]
    StorageRecords {
        id: u64,
        /// Session records as JSON array.
        records: Vec<serde_json::Value>,
    },
    /// Storage operation completed (store/delete acknowledgement).
    #[serde(rename = "storage.ok")]
    StorageOk { id: u64 },
    /// Error response for any request.
    Error { id: u64, message: String },
}

impl PluginResponse {
    /// Get the response ID for request correlation.
    #[must_use]
    pub fn id(&self) -> u64 {
        match self {
            Self::InitOk { id, .. }
            | Self::ActivateOk { id }
            | Self::DeactivateOk { id }
            | Self::GateFinding { id, .. }
            | Self::GateDone { id, .. }
            | Self::WorkflowStepProgress { id, .. }
            | Self::WorkflowStepFinding { id, .. }
            | Self::WorkflowStepDone { id, .. }
            | Self::StorageRecord { id, .. }
            | Self::StorageRecords { id, .. }
            | Self::StorageOk { id }
            | Self::Error { id, .. } => *id,
        }
    }

    /// Whether this response indicates an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

/// Where a discovered plugin was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginScope {
    /// Found in the project-local `.magistr/plugins/` directory.
    ProjectLocal,
    /// Found in the global `~/.config/magistr/plugins/` directory.
    Global,
}

impl std::fmt::Display for PluginScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectLocal => f.write_str("project-local"),
            Self::Global => f.write_str("global"),
        }
    }
}

/// A plugin discovered during filesystem scanning.
///
/// Combines the manifest with its location metadata.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// The parsed manifest.
    pub manifest: PluginManifest,
    /// Absolute path to the plugin directory.
    pub path: std::path::PathBuf,
    /// Whether this is a project-local or global plugin.
    pub scope: PluginScope,
}

/// Errors from plugin operations.
#[derive(Debug)]
pub enum PluginError {
    /// TOML parse error on manifest.
    ManifestParse(String),
    /// TOML serialization error.
    ManifestSerialize(String),
    /// Manifest file not found in plugin directory.
    ManifestNotFound(String),
    /// Manifest failed validation (e.g., missing entrypoint for executable plugin).
    ManifestValidation(Vec<String>),
    /// Plugin directory not found.
    DirNotFound(String),
    /// Plugin already installed.
    AlreadyInstalled(String),
    /// Plugin not found in registry.
    NotFound(String),
    /// File I/O error.
    Io(String, std::io::Error),
    /// Git clone error.
    GitError(String),
    /// Plugin subprocess failed to start.
    ProcessStart(String),
    /// Plugin subprocess communication error (stdin/stdout).
    ProcessIo(String),
    /// Plugin returned an error response.
    PluginError { id: u64, message: String },
    /// Plugin subprocess exited unexpectedly.
    ProcessExited(String),
    /// Plugin lifecycle violation (e.g., activate before init).
    LifecycleViolation(String),
    /// Timeout waiting for plugin response.
    Timeout(String),
    /// Failed to fetch the plugin registry index.
    IndexFetch(String),
    /// Failed to parse the plugin registry index.
    IndexParse(String),
    /// Plugin not found in the registry index.
    IndexNotFound(String),
    /// Failed to download a plugin archive.
    DownloadFailed(String),
    /// Plugin archive extraction failed.
    ArchiveError(String),
    /// Plugin signature verification failed.
    SignatureInvalid(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestParse(e) => write!(f, "failed to parse plugin manifest: {e}"),
            Self::ManifestSerialize(e) => write!(f, "failed to serialize plugin manifest: {e}"),
            Self::ManifestNotFound(dir) => {
                write!(f, "no {MANIFEST_FILENAME} found in '{dir}'")
            }
            Self::ManifestValidation(errors) => {
                write!(f, "manifest validation failed: {}", errors.join("; "))
            }
            Self::DirNotFound(dir) => write!(f, "plugin directory not found: '{dir}'"),
            Self::AlreadyInstalled(name) => {
                write!(f, "plugin '{name}' is already installed")
            }
            Self::NotFound(name) => write!(f, "plugin '{name}' not found"),
            Self::Io(path, e) => write!(f, "plugin I/O error at '{path}': {e}"),
            Self::GitError(msg) => write!(f, "git error: {msg}"),
            Self::ProcessStart(msg) => write!(f, "failed to start plugin process: {msg}"),
            Self::ProcessIo(msg) => write!(f, "plugin I/O error: {msg}"),
            Self::PluginError { id, message } => {
                write!(f, "plugin error (request {id}): {message}")
            }
            Self::ProcessExited(msg) => write!(f, "plugin process exited: {msg}"),
            Self::LifecycleViolation(msg) => write!(f, "plugin lifecycle violation: {msg}"),
            Self::Timeout(msg) => write!(f, "plugin timeout: {msg}"),
            Self::IndexFetch(msg) => write!(f, "failed to fetch plugin index: {msg}"),
            Self::IndexParse(msg) => write!(f, "failed to parse plugin index: {msg}"),
            Self::IndexNotFound(name) => {
                write!(f, "plugin '{name}' not found in registry index")
            }
            Self::DownloadFailed(msg) => write!(f, "failed to download plugin: {msg}"),
            Self::ArchiveError(msg) => write!(f, "plugin archive error: {msg}"),
            Self::SignatureInvalid(msg) => {
                write!(f, "plugin signature verification failed: {msg}")
            }
        }
    }
}

impl std::error::Error for PluginError {}

#[cfg(test)]
#[path = "types.tests.rs"]
mod tests;
