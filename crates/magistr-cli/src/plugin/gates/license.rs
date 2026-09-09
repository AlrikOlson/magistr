//! License compliance quality gate plugin — checks dependency licenses.
//!
//! Scans dependency manifests (Cargo.toml, package.json, etc.) for license
//! declarations and checks them against configurable allow/deny lists.

use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Instant;

use crate::plugin::gate_trait::{GateConfig, GateFinding, GateOutcome, GatePlugin, GateSeverity};
use crate::plugin::types::PluginError;

/// Configuration for the license compliance gate.
#[derive(Debug, Clone)]
pub struct LicenseConfig {
    /// Licenses that are allowed (e.g., "MIT", "Apache-2.0", "BSD-3-Clause").
    /// If non-empty, only these licenses are permitted.
    pub allowed: HashSet<String>,
    /// Licenses that are explicitly denied (e.g., "GPL-3.0", "AGPL-3.0").
    /// Checked after the allow list.
    pub denied: HashSet<String>,
    /// Whether to treat missing license declarations as errors.
    pub deny_unlicensed: bool,
}

impl Default for LicenseConfig {
    fn default() -> Self {
        Self {
            allowed: HashSet::new(),
            denied: ["GPL-3.0", "AGPL-3.0", "GPL-3.0-only", "AGPL-3.0-only"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            deny_unlicensed: true,
        }
    }
}

impl LicenseConfig {
    /// Parse from a JSON value (the `config` field of [`GateConfig`]).
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut cfg = Self::default();
        if let Some(allowed) = value.get("allowed").and_then(|v| v.as_array()) {
            cfg.allowed = allowed
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(denied) = value.get("denied").and_then(|v| v.as_array()) {
            cfg.denied = denied
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(deny) = value.get("deny_unlicensed").and_then(|v| v.as_bool()) {
            cfg.deny_unlicensed = deny;
        }
        cfg
    }
}

/// A detected dependency with license information.
#[derive(Debug, Clone)]
pub struct DependencyLicense {
    /// Dependency name (e.g., "serde").
    pub name: String,
    /// Detected license identifier (e.g., "MIT", "Apache-2.0").
    pub license: Option<String>,
    /// Which manifest file declared this dependency.
    pub source_file: String,
}

/// Native license compliance quality gate plugin.
pub struct LicenseGate {
    /// Default configuration (can be overridden per-run via [`GateConfig::config`]).
    default_config: LicenseConfig,
}

impl LicenseGate {
    /// Create a new license gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_config: LicenseConfig::default(),
        }
    }

    /// Create with explicit configuration.
    #[must_use]
    pub fn with_config(config: LicenseConfig) -> Self {
        Self {
            default_config: config,
        }
    }

    /// Scan the project directory for dependency license information.
    fn scan_licenses(project_dir: &Path) -> Vec<DependencyLicense> {
        let mut deps = Vec::new();

        // Try Cargo.toml (Rust)
        let cargo_toml = project_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            deps.extend(Self::scan_cargo_toml(&cargo_toml));
        }

        // Try package.json (Node.js)
        let package_json = project_dir.join("package.json");
        if package_json.exists() {
            deps.extend(Self::scan_package_json(&package_json));
        }

        deps
    }

    /// Parse Cargo.toml for dependency names.
    ///
    /// Note: Cargo.toml doesn't contain license info for dependencies —
    /// that's in Cargo.lock / crates.io metadata. This implementation
    /// detects deps and reports them as needing verification. A full
    /// implementation would use `cargo metadata` or `cargo-deny`.
    fn scan_cargo_toml(path: &Path) -> Vec<DependencyLicense> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let source_file = path.display().to_string();
        let mut deps = Vec::new();

        // Simple TOML parsing for [dependencies] section.
        let mut in_deps = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[dependencies]")
                || trimmed.starts_with("[dev-dependencies]")
                || trimmed.starts_with("[build-dependencies]")
            {
                in_deps = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_deps = false;
                continue;
            }
            if in_deps && let Some((name, _)) = trimmed.split_once('=') {
                let name = name.trim().to_string();
                if !name.is_empty() && !name.starts_with('#') {
                    deps.push(DependencyLicense {
                        name,
                        license: None, // Would need cargo metadata for actual licenses
                        source_file: source_file.clone(),
                    });
                }
            }
        }

        deps
    }

    /// Parse package.json for dependency license info.
    fn scan_package_json(path: &Path) -> Vec<DependencyLicense> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let source_file = path.display().to_string();
        let mut deps = Vec::new();

        // The project's own license
        let project_license = json
            .get("license")
            .and_then(|v| v.as_str())
            .map(String::from);
        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
            deps.push(DependencyLicense {
                name: name.to_string(),
                license: project_license,
                source_file: source_file.clone(),
            });
        }

        // Scan dependencies and devDependencies for names (licenses not in package.json).
        for section in &["dependencies", "devDependencies"] {
            if let Some(obj) = json.get(section).and_then(|v| v.as_object()) {
                for name in obj.keys() {
                    deps.push(DependencyLicense {
                        name: name.clone(),
                        license: None,
                        source_file: source_file.clone(),
                    });
                }
            }
        }

        deps
    }

    /// Check a single dependency against the allow/deny lists.
    fn check_dependency(dep: &DependencyLicense, config: &LicenseConfig) -> Option<GateFinding> {
        let Some(ref license) = dep.license else {
            if config.deny_unlicensed {
                return Some(GateFinding {
                    file: Some(dep.source_file.clone()),
                    line: None,
                    message: format!("dependency '{}' has no license declaration", dep.name),
                    severity: GateSeverity::Warning,
                    rule_id: Some("license-missing".to_string()),
                });
            }
            return None;
        };

        // Check deny list first.
        if config.denied.contains(license) {
            return Some(GateFinding {
                file: Some(dep.source_file.clone()),
                line: None,
                message: format!("dependency '{}' uses denied license '{license}'", dep.name),
                severity: GateSeverity::Error,
                rule_id: Some(format!("license-denied:{license}")),
            });
        }

        // Check allow list (if non-empty, only listed licenses are OK).
        if !config.allowed.is_empty() && !config.allowed.contains(license) {
            return Some(GateFinding {
                file: Some(dep.source_file.clone()),
                line: None,
                message: format!(
                    "dependency '{}' uses non-allowed license '{license}'",
                    dep.name
                ),
                severity: GateSeverity::Error,
                rule_id: Some(format!("license-not-allowed:{license}")),
            });
        }

        None
    }
}

impl Default for LicenseGate {
    fn default() -> Self {
        Self::new()
    }
}

impl GatePlugin for LicenseGate {
    fn run_gate(
        &mut self,
        config: GateConfig,
    ) -> Pin<Box<dyn Future<Output = Result<GateOutcome, PluginError>> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();

            let license_config = config
                .config
                .as_ref()
                .map(LicenseConfig::from_json)
                .unwrap_or_else(|| self.default_config.clone());

            let project_dir = PathBuf::from(&config.project_dir);
            let deps = Self::scan_licenses(&project_dir);

            let findings: Vec<GateFinding> = deps
                .iter()
                .filter_map(|dep| Self::check_dependency(dep, &license_config))
                .collect();

            let error_count = findings
                .iter()
                .filter(|f| f.severity == GateSeverity::Error)
                .count();
            let warning_count = findings
                .iter()
                .filter(|f| f.severity == GateSeverity::Warning)
                .count();
            let passed = error_count == 0;

            let summary = if passed && warning_count == 0 {
                format!("License check passed ({} dependencies scanned)", deps.len())
            } else if passed {
                format!(
                    "License check passed with {warning_count} warnings ({} dependencies scanned)",
                    deps.len()
                )
            } else {
                format!(
                    "License check failed: {error_count} errors, {warning_count} warnings ({} dependencies scanned)",
                    deps.len()
                )
            };

            Ok(GateOutcome {
                passed,
                gate_name: config.gate_name,
                findings,
                summary,
                duration: start.elapsed(),
            })
        })
    }

    fn name(&self) -> &str {
        "license"
    }

    fn supported_gates(&self) -> &[String] {
        static GATES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        GATES.get_or_init(|| vec!["license".to_string()])
    }
}

#[cfg(test)]
#[path = "license.tests.rs"]
mod tests;
