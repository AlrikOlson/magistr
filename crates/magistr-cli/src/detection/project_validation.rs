//! Project validation using the same check selection and execution as MCP.

use std::path::Path;
use std::time::Duration;

use magistr_engine::policy::GovernancePolicy;

use crate::error::{MagistrError, Result};
use crate::output::envelope::{ValidationCheckOutput, ValidationOutput};
use crate::utils::check::{CheckRun, run_check};

/// Results for standard, archetype, and required custom project checks.
pub(crate) struct ProjectValidation {
    pub(crate) results: Vec<CheckRun>,
    pub(crate) checks: Vec<magistr_core::checks::CheckSpec>,
}

impl ProjectValidation {
    pub(crate) async fn run(dir: &Path) -> Result<Self> {
        let config = super::load_project_config(dir)?;
        let specs = crate::mcp_tools::check::resolve_checks(&config, "all")
            .map_err(MagistrError::Config)?;
        let mut handles = Vec::with_capacity(specs.len());
        for spec in specs {
            let dir = dir.to_path_buf();
            handles.push(tokio::spawn(async move {
                run_check(
                    &spec.name,
                    &spec.command,
                    spec.required,
                    &dir,
                    0,
                    &tokio_util::sync::CancellationToken::new(),
                    |_| {},
                )
                .await
            }));
        }
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(
                handle
                    .await
                    .map_err(|e| MagistrError::TaskJoin(e.to_string()))?,
            );
        }
        let checks =
            magistr_core::checks::resolve_checks(&config, "check").map_err(MagistrError::Config)?;
        Ok(Self { results, checks })
    }

    pub(crate) fn all_passed(&self) -> bool {
        self.results.iter().all(CheckRun::passed)
    }

    pub(crate) fn passed_count(&self) -> usize {
        self.results.iter().filter(|check| check.passed()).count()
    }

    pub(crate) fn failed_count(&self) -> usize {
        self.results.len() - self.passed_count()
    }

    pub(crate) fn total_duration(&self) -> Duration {
        self.results.iter().map(|check| check.duration).sum()
    }

    /// A standard check is satisfied only when every resolved member passed.
    pub(crate) fn policy_violations(&self, policy: &GovernancePolicy) -> Vec<String> {
        if policy.allow_check_bypass {
            return Vec::new();
        }
        let satisfied: Vec<&str> = policy
            .required_checks
            .iter()
            .filter(|name| {
                if name.as_str() == "check" && !self.checks.is_empty() {
                    return self.checks.iter().all(|check| {
                        self.results.iter().any(|actual| {
                            actual.passed()
                                && !actual.command.is_empty()
                                && (actual.name == check.name
                                    || magistr_core::checks::clippy_covers(
                                        &check.command,
                                        &actual.command,
                                    ))
                        })
                    });
                }
                let prefix = format!("{name}:");
                let mut matching = self
                    .results
                    .iter()
                    .filter(|check| check.name == **name || check.name.starts_with(&prefix))
                    .peekable();
                matching.peek().is_some()
                    && matching.all(|check| check.passed() && !check.command.is_empty())
            })
            .map(String::as_str)
            .collect();
        policy.missing_required_checks(&satisfied)
    }
}

impl From<&ProjectValidation> for ValidationOutput {
    fn from(result: &ProjectValidation) -> Self {
        Self {
            all_passed: result.all_passed(),
            checks: result
                .results
                .iter()
                .map(|check| ValidationCheckOutput {
                    name: check.name.clone(),
                    passed: check.passed(),
                    duration_ms: check.duration.as_millis() as u64,
                    command: check.command.clone(),
                })
                .collect(),
            total_duration_ms: result.total_duration().as_millis() as u64,
        }
    }
}

#[cfg(test)]
#[path = "project_validation.tests.rs"]
mod tests;
