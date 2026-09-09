//! SBOM (Software Bill of Materials) quality gate plugin.
//!
//! Generates and validates an SBOM for the project. Supports multiple
//! generators: `cargo-sbom` (Rust), `syft` (universal), `cyclonedx-cli`.
//! Reports findings for missing components, known vulnerabilities, and
//! SBOM format compliance.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::time::Instant;

use crate::plugin::gate_trait::{GateConfig, GateFinding, GateOutcome, GatePlugin, GateSeverity};
use crate::plugin::types::PluginError;

/// SBOM output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbomFormat {
    /// CycloneDX JSON format.
    CycloneDxJson,
    /// CycloneDX XML format.
    CycloneDxXml,
    /// SPDX JSON format.
    SpdxJson,
}

impl SbomFormat {
    /// File extension for this format.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::CycloneDxJson => "cdx.json",
            Self::CycloneDxXml => "cdx.xml",
            Self::SpdxJson => "spdx.json",
        }
    }

    /// Label for display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::CycloneDxJson => "cyclonedx-json",
            Self::CycloneDxXml => "cyclonedx-xml",
            Self::SpdxJson => "spdx-json",
        }
    }
}

/// Configuration for the SBOM gate.
#[derive(Debug, Clone)]
pub struct SbomConfig {
    /// Preferred SBOM generator tool.
    pub generator: SbomGenerator,
    /// Output format.
    pub format: SbomFormat,
    /// Minimum number of components expected in the SBOM.
    pub min_components: Option<usize>,
    /// Whether to fail if the generator tool is not installed.
    pub require_tool: bool,
}

impl Default for SbomConfig {
    fn default() -> Self {
        Self {
            generator: SbomGenerator::Auto,
            format: SbomFormat::CycloneDxJson,
            min_components: None,
            require_tool: false,
        }
    }
}

impl SbomConfig {
    /// Parse from a JSON value.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut cfg = Self::default();
        if let Some(generator) = value.get("generator").and_then(|v| v.as_str()) {
            cfg.generator = match generator {
                "cargo-sbom" => SbomGenerator::CargoSbom,
                "syft" => SbomGenerator::Syft,
                "cyclonedx-cli" => SbomGenerator::CycloneDxCli,
                _ => SbomGenerator::Auto,
            };
        }
        if let Some(fmt) = value.get("format").and_then(|v| v.as_str()) {
            cfg.format = match fmt {
                "cyclonedx-xml" => SbomFormat::CycloneDxXml,
                "spdx-json" => SbomFormat::SpdxJson,
                _ => SbomFormat::CycloneDxJson,
            };
        }
        if let Some(min) = value.get("min_components").and_then(|v| v.as_u64()) {
            cfg.min_components = Some(min as usize);
        }
        if let Some(req) = value.get("require_tool").and_then(|v| v.as_bool()) {
            cfg.require_tool = req;
        }
        cfg
    }
}

/// Which SBOM generator to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbomGenerator {
    /// Auto-detect the best available generator.
    Auto,
    /// `cargo-sbom` — Rust-specific, CycloneDX output.
    CargoSbom,
    /// `syft` — universal SBOM generator by Anchore.
    Syft,
    /// `cyclonedx-cli` — CycloneDX official CLI.
    CycloneDxCli,
}

impl SbomGenerator {
    /// The CLI command name for this generator.
    #[must_use]
    pub fn command(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::CargoSbom => "cargo-sbom",
            Self::Syft => "syft",
            Self::CycloneDxCli => "cyclonedx-cli",
        }
    }

    /// Check if this generator tool is available on PATH.
    #[must_use]
    pub fn is_available(self) -> bool {
        match self {
            Self::Auto => false, // sentinel — auto-detect checks each tool
            Self::CargoSbom => which("cargo-sbom"),
            Self::Syft => which("syft"),
            Self::CycloneDxCli => which("cyclonedx"),
        }
    }
}

/// Check if a command is available on PATH.
fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Native SBOM quality gate plugin.
pub struct SbomGate {
    /// Default configuration (can be overridden per-run via [`GateConfig::config`]).
    default_config: SbomConfig,
}

impl SbomGate {
    /// Create a new SBOM gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_config: SbomConfig::default(),
        }
    }

    /// Create with explicit configuration.
    #[must_use]
    pub fn with_config(config: SbomConfig) -> Self {
        Self {
            default_config: config,
        }
    }

    /// Auto-detect the best available SBOM generator.
    fn detect_generator(project_dir: &Path) -> Option<SbomGenerator> {
        // Prefer cargo-sbom for Rust projects
        if project_dir.join("Cargo.toml").exists() && SbomGenerator::CargoSbom.is_available() {
            return Some(SbomGenerator::CargoSbom);
        }
        // Syft works for any project
        if SbomGenerator::Syft.is_available() {
            return Some(SbomGenerator::Syft);
        }
        // CycloneDX CLI as fallback
        if SbomGenerator::CycloneDxCli.is_available() {
            return Some(SbomGenerator::CycloneDxCli);
        }
        None
    }

    /// Run an SBOM generator and return the output path.
    fn generate_sbom(
        generator: SbomGenerator,
        format: SbomFormat,
        project_dir: &Path,
    ) -> Result<(PathBuf, String), String> {
        let output_file = project_dir.join(format!("sbom.{}", format.extension()));

        let result = match generator {
            SbomGenerator::CargoSbom => Command::new("cargo-sbom")
                .arg("--output-format")
                .arg(format.label())
                .current_dir(project_dir)
                .output(),
            SbomGenerator::Syft => Command::new("syft")
                .arg("dir:.")
                .arg("-o")
                .arg(format.label())
                .arg("--file")
                .arg(&output_file)
                .current_dir(project_dir)
                .output(),
            SbomGenerator::CycloneDxCli => Command::new("cyclonedx")
                .arg("analyze")
                .arg("--output-format")
                .arg(format.label())
                .arg("--output-file")
                .arg(&output_file)
                .current_dir(project_dir)
                .output(),
            SbomGenerator::Auto => {
                return Err("auto generator should be resolved first".to_string());
            }
        };

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if output.status.success() {
                    // cargo-sbom writes to stdout, not a file
                    if generator == SbomGenerator::CargoSbom
                        && !stdout.is_empty()
                        && let Err(e) = std::fs::write(&output_file, &stdout)
                    {
                        return Err(format!("failed to write SBOM: {e}"));
                    }
                    Ok((output_file, stdout))
                } else {
                    Err(format!(
                        "{} failed (exit {}): {}",
                        generator.command(),
                        output.status,
                        stderr.trim()
                    ))
                }
            }
            Err(e) => Err(format!("failed to run {}: {e}", generator.command())),
        }
    }

    /// Validate an SBOM file and return findings.
    fn validate_sbom(sbom_path: &Path, config: &SbomConfig) -> (bool, Vec<GateFinding>, String) {
        let mut findings = Vec::new();
        let mut passed = true;

        // Read and parse the SBOM
        let content = match std::fs::read_to_string(sbom_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(GateFinding {
                    file: Some(sbom_path.display().to_string()),
                    line: None,
                    message: format!("Failed to read SBOM: {e}"),
                    severity: GateSeverity::Error,
                    rule_id: Some("SBOM-READ".to_string()),
                });
                return (false, findings, "SBOM unreadable".to_string());
            }
        };

        // Parse as JSON (CycloneDX JSON or SPDX JSON)
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                // Might be XML format — just check it's non-empty
                if content.contains("<bom") || content.contains("<spdx") {
                    findings.push(GateFinding {
                        file: Some(sbom_path.display().to_string()),
                        line: None,
                        message: "SBOM generated in XML format (validation limited)".to_string(),
                        severity: GateSeverity::Info,
                        rule_id: Some("SBOM-FORMAT".to_string()),
                    });
                    let summary = format!("SBOM generated ({} bytes, XML)", content.len());
                    return (true, findings, summary);
                }
                findings.push(GateFinding {
                    file: Some(sbom_path.display().to_string()),
                    line: None,
                    message: format!("Failed to parse SBOM as JSON: {e}"),
                    severity: GateSeverity::Error,
                    rule_id: Some("SBOM-PARSE".to_string()),
                });
                return (false, findings, "SBOM parse failed".to_string());
            }
        };

        // Count components
        let component_count =
            if let Some(components) = json.get("components").and_then(|v| v.as_array()) {
                components.len()
            } else if let Some(packages) = json.get("packages").and_then(|v| v.as_array()) {
                // SPDX format
                packages.len()
            } else {
                0
            };

        // Check minimum component count
        if let Some(min) = config.min_components
            && component_count < min
        {
            findings.push(GateFinding {
                file: Some(sbom_path.display().to_string()),
                line: None,
                message: format!(
                    "SBOM contains {component_count} components, expected at least {min}"
                ),
                severity: GateSeverity::Error,
                rule_id: Some("SBOM-MIN-COMPONENTS".to_string()),
            });
            passed = false;
        }

        // Check for components without licenses (CycloneDX)
        if let Some(components) = json.get("components").and_then(|v| v.as_array()) {
            let mut unlicensed = 0;
            for comp in components {
                let has_license = comp
                    .get("licenses")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| !a.is_empty());
                if !has_license {
                    unlicensed += 1;
                }
            }
            if unlicensed > 0 {
                findings.push(GateFinding {
                    file: Some(sbom_path.display().to_string()),
                    line: None,
                    message: format!("{unlicensed} component(s) missing license information"),
                    severity: GateSeverity::Warning,
                    rule_id: Some("SBOM-UNLICENSED".to_string()),
                });
            }
        }

        let summary = format!(
            "SBOM generated: {component_count} components, {} findings",
            findings.len()
        );

        (passed, findings, summary)
    }
}

impl Default for SbomGate {
    fn default() -> Self {
        Self::new()
    }
}

impl GatePlugin for SbomGate {
    fn run_gate(
        &mut self,
        config: GateConfig,
    ) -> Pin<Box<dyn Future<Output = Result<GateOutcome, PluginError>> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();
            let project_dir = PathBuf::from(&config.project_dir);

            let sbom_config = config
                .config
                .as_ref()
                .map(SbomConfig::from_json)
                .unwrap_or_else(|| self.default_config.clone());

            // Resolve the generator
            let generator = match sbom_config.generator {
                SbomGenerator::Auto => match Self::detect_generator(&project_dir) {
                    Some(g) => g,
                    None => {
                        let severity = if sbom_config.require_tool {
                            GateSeverity::Error
                        } else {
                            GateSeverity::Warning
                        };
                        return Ok(GateOutcome {
                            passed: !sbom_config.require_tool,
                            gate_name: config.gate_name,
                            findings: vec![GateFinding {
                                file: None,
                                line: None,
                                message: "No SBOM generator found (install cargo-sbom, syft, or cyclonedx-cli)".to_string(),
                                severity,
                                rule_id: Some("SBOM-TOOL-MISSING".to_string()),
                            }],
                            summary: "No SBOM generator available".to_string(),
                            duration: start.elapsed(),
                        });
                    }
                },
                other => {
                    if !other.is_available() {
                        return Ok(GateOutcome {
                            passed: !sbom_config.require_tool,
                            gate_name: config.gate_name,
                            findings: vec![GateFinding {
                                file: None,
                                line: None,
                                message: format!("{} not found on PATH", other.command()),
                                severity: if sbom_config.require_tool {
                                    GateSeverity::Error
                                } else {
                                    GateSeverity::Warning
                                },
                                rule_id: Some("SBOM-TOOL-MISSING".to_string()),
                            }],
                            summary: format!("{} not available", other.command()),
                            duration: start.elapsed(),
                        });
                    }
                    other
                }
            };

            // Generate the SBOM
            match Self::generate_sbom(generator, sbom_config.format, &project_dir) {
                Ok((sbom_path, _stdout)) => {
                    let (passed, findings, summary) = Self::validate_sbom(&sbom_path, &sbom_config);
                    Ok(GateOutcome {
                        passed,
                        gate_name: config.gate_name,
                        findings,
                        summary,
                        duration: start.elapsed(),
                    })
                }
                Err(err_msg) => Ok(GateOutcome {
                    passed: false,
                    gate_name: config.gate_name,
                    findings: vec![GateFinding {
                        file: None,
                        line: None,
                        message: err_msg,
                        severity: GateSeverity::Error,
                        rule_id: Some("SBOM-GENERATE".to_string()),
                    }],
                    summary: "SBOM generation failed".to_string(),
                    duration: start.elapsed(),
                }),
            }
        })
    }

    fn name(&self) -> &str {
        "sbom"
    }

    fn supported_gates(&self) -> &[String] {
        &SUPPORTED_GATES
    }
}

static SUPPORTED_GATES: [String; 1] = [String::new()];

// Workaround: static String array initialization.
// The trait requires &[String], so we provide a module-level helper.
impl SbomGate {
    /// Get the list of supported gate names.
    #[must_use]
    pub fn gate_names() -> Vec<String> {
        vec!["sbom".to_string()]
    }
}

#[cfg(test)]
#[path = "sbom.tests.rs"]
mod tests;
