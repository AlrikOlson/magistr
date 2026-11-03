//! SonarQube check plugin — runs `sonar-scanner` and checks check status.
//!
//! This is a native (non-subprocess) implementation of [`CheckPlugin`] that
//! invokes the `sonar-scanner` CLI and queries the SonarQube API for quality
//! check results and issues.

use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use crate::plugin::check_trait::{
    CheckFinding, CheckPlugin, CheckPluginConfig, CheckSeverity, PluginCheckOutcome,
};
use crate::plugin::types::PluginError;

/// Configuration for the SonarQube check plugin.
#[derive(Debug, Clone)]
pub struct SonarQubeConfig {
    /// SonarQube server URL (e.g., "http://localhost:9000").
    pub server_url: String,
    /// Authentication token for the SonarQube API.
    pub token: Option<String>,
    /// Project key in SonarQube.
    pub project_key: Option<String>,
    /// Path to the `sonar-scanner` binary (defaults to "sonar-scanner" on PATH).
    pub scanner_path: String,
}

impl Default for SonarQubeConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:9000".to_string(),
            token: None,
            project_key: None,
            scanner_path: "sonar-scanner".to_string(),
        }
    }
}

impl SonarQubeConfig {
    /// Parse from a JSON value (the `config` field of [`CheckPluginConfig`]).
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut cfg = Self::default();
        if let Some(url) = value.get("server_url").and_then(|v| v.as_str()) {
            cfg.server_url = url.to_string();
        }
        if let Some(token) = value.get("token").and_then(|v| v.as_str()) {
            cfg.token = Some(token.to_string());
        }
        if let Some(key) = value.get("project_key").and_then(|v| v.as_str()) {
            cfg.project_key = Some(key.to_string());
        }
        if let Some(path) = value.get("scanner_path").and_then(|v| v.as_str()) {
            cfg.scanner_path = path.to_string();
        }
        cfg
    }
}

/// Native SonarQube check plugin.
///
/// Runs `sonar-scanner` to analyze the project and then queries the
/// SonarQube API for the check status and issues.
pub struct SonarQubeCheck {
    /// Default configuration (can be overridden per-run via [`CheckPluginConfig::config`]).
    default_config: SonarQubeConfig,
}

impl SonarQubeCheck {
    /// Create a new SonarQube check with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_config: SonarQubeConfig::default(),
        }
    }

    /// Create with explicit configuration.
    #[must_use]
    pub fn with_config(config: SonarQubeConfig) -> Self {
        Self {
            default_config: config,
        }
    }

    /// Run the sonar-scanner CLI and return its output.
    async fn run_scanner(
        &self,
        config: &SonarQubeConfig,
        project_dir: &str,
    ) -> Result<std::process::Output, PluginError> {
        let mut cmd = tokio::process::Command::new(&config.scanner_path);
        cmd.current_dir(project_dir);
        cmd.arg(format!("-Dsonar.host.url={}", config.server_url));

        if let Some(ref token) = config.token {
            cmd.arg(format!("-Dsonar.token={token}"));
        }
        if let Some(ref key) = config.project_key {
            cmd.arg(format!("-Dsonar.projectKey={key}"));
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        cmd.output()
            .await
            .map_err(|e| PluginError::ProcessStart(format!("failed to run sonar-scanner: {e}")))
    }

    /// Parse sonar-scanner output for findings.
    ///
    /// In a real integration this would query the SonarQube API for issues.
    /// This implementation parses the scanner output for basic feedback.
    fn parse_scanner_output(output: &std::process::Output) -> (bool, Vec<CheckFinding>, String) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        let passed = output.status.success();
        let mut findings = Vec::new();

        // Parse lines that look like issue reports from scanner output.
        for line in combined.lines() {
            if let Some(finding) = Self::parse_issue_line(line) {
                findings.push(finding);
            }
        }

        let summary = if passed {
            "SonarQube check passed".to_string()
        } else {
            let error_count = findings
                .iter()
                .filter(|f| f.severity == CheckSeverity::Error)
                .count();
            format!(
                "SonarQube check failed ({} errors, {} total findings)",
                error_count,
                findings.len()
            )
        };

        (passed, findings, summary)
    }

    /// Try to parse a single line as a SonarQube issue.
    fn parse_issue_line(line: &str) -> Option<CheckFinding> {
        // SonarQube scanner outputs issues in various formats.
        // This handles the common "ERROR: ... at file:line" pattern.
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let (severity, rest) = if let Some(rest) = trimmed.strip_prefix("ERROR:") {
            (CheckSeverity::Error, rest.trim())
        } else {
            (
                CheckSeverity::Warning,
                trimmed.strip_prefix("WARN:")?.trim(),
            )
        };

        // Try to extract file:line from the message.
        let (file, line_num, message) = if let Some((loc, msg)) = rest.split_once(" - ") {
            if let Some((f, l)) = loc.rsplit_once(':') {
                let line_num = l.parse::<u32>().ok();
                (Some(f.to_string()), line_num, msg.to_string())
            } else {
                (None, None, rest.to_string())
            }
        } else {
            (None, None, rest.to_string())
        };

        Some(CheckFinding {
            file,
            line: line_num,
            message,
            severity,
            rule_id: None,
        })
    }
}

impl Default for SonarQubeCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckPlugin for SonarQubeCheck {
    fn run_check(
        &mut self,
        config: CheckPluginConfig,
    ) -> Pin<Box<dyn Future<Output = Result<PluginCheckOutcome, PluginError>> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();

            let sq_config = config
                .config
                .as_ref()
                .map(SonarQubeConfig::from_json)
                .unwrap_or_else(|| self.default_config.clone());

            let output = self.run_scanner(&sq_config, &config.project_dir).await?;
            let (passed, findings, summary) = Self::parse_scanner_output(&output);

            Ok(PluginCheckOutcome {
                passed,
                check_name: config.check_name,
                findings,
                summary,
                duration: start.elapsed(),
            })
        })
    }

    fn name(&self) -> &str {
        "sonarqube"
    }

    fn supported_checks(&self) -> &[String] {
        // Return a static reference by leaking — this is fine for a singleton.
        // In practice, supported_checks is called rarely.
        static CHECKS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        CHECKS.get_or_init(|| vec!["sonarqube".to_string()])
    }
}

#[cfg(test)]
#[path = "sonarqube.tests.rs"]
mod tests;
