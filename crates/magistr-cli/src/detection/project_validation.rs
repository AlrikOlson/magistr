//! Project validation using the same gate selection and execution as MCP.

use std::path::Path;
use std::time::Duration;

use magistr_engine::policy::GovernancePolicy;

use crate::error::{MagistrError, Result};
use crate::output::envelope::{GateOutput, ValidationOutput};
use crate::utils::gate::{GateRun, run_gate};

/// Results for standard, archetype, and required custom project gates.
pub(crate) struct ProjectValidation {
    pub(crate) results: Vec<GateRun>,
}

impl ProjectValidation {
    pub(crate) async fn run(dir: &Path) -> Result<Self> {
        let config = super::load_project_config(dir)?;
        let specs =
            crate::mcp_tools::gate::resolve_gates(&config, "all").map_err(MagistrError::Config)?;
        let mut handles = Vec::with_capacity(specs.len());
        for spec in specs {
            let dir = dir.to_path_buf();
            handles.push(tokio::spawn(async move {
                run_gate(
                    &spec.name,
                    &spec.command,
                    spec.required,
                    &dir,
                    300,
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
        Ok(Self { results })
    }

    pub(crate) fn all_passed(&self) -> bool {
        self.results.iter().all(GateRun::passed)
    }

    pub(crate) fn passed_count(&self) -> usize {
        self.results.iter().filter(|gate| gate.passed()).count()
    }

    pub(crate) fn failed_count(&self) -> usize {
        self.results.len() - self.passed_count()
    }

    pub(crate) fn total_duration(&self) -> Duration {
        self.results.iter().map(|gate| gate.duration).sum()
    }

    /// A standard gate is satisfied only when every resolved member passed.
    pub(crate) fn policy_violations(&self, policy: &GovernancePolicy) -> Vec<String> {
        if policy.allow_gate_bypass {
            return Vec::new();
        }
        let satisfied: Vec<&str> = policy
            .required_gates
            .iter()
            .filter(|name| {
                let prefix = format!("{name}:");
                let mut matching = self
                    .results
                    .iter()
                    .filter(|gate| gate.name == **name || gate.name.starts_with(&prefix))
                    .peekable();
                matching.peek().is_some()
                    && matching.all(|gate| gate.passed() && !gate.command.is_empty())
            })
            .map(String::as_str)
            .collect();
        policy.missing_required_gates(&satisfied)
    }
}

impl From<&ProjectValidation> for ValidationOutput {
    fn from(result: &ProjectValidation) -> Self {
        Self {
            all_passed: result.all_passed(),
            gates: result
                .results
                .iter()
                .map(|gate| GateOutput {
                    name: gate.name.clone(),
                    passed: gate.passed(),
                    duration_ms: gate.duration.as_millis() as u64,
                    command: gate.command.clone(),
                })
                .collect(),
            total_duration_ms: result.total_duration().as_millis() as u64,
        }
    }
}

#[cfg(test)]
#[path = "project_validation.tests.rs"]
mod tests;
