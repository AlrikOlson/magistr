//! One budget and resource policy for all verification entry points.
use crate::types::{GatePolicy, ProjectConfig, VerificationConfig};
use magistr_process::GateSchedule;
use std::path::Path;

/// Resolve an override only for its actual configured command.
pub fn configured_policy(
    config: &ProjectConfig,
    name: &str,
    command: Option<&str>,
) -> Option<GatePolicy> {
    super::resolve_gates(config, name)
        .ok()?
        .into_iter()
        .find(|spec| Some(spec.command.trim()) == command.map(str::trim))
        .and_then(|spec| config.verification.gates.get(&spec.name).cloned())
}

/// The same read-only settings used by native checks, MCP gates and the loop.
pub fn verification_policy(
    root: &Path,
    name: &str,
    command: Option<&str>,
) -> Result<(VerificationConfig, Option<GatePolicy>), String> {
    if !root.exists() {
        return Err(format!("The workspace {} does not exist.", root.display()));
    }
    let path = crate::paths::project_json(root).map_err(|error| error.to_string())?;
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Default::default(), None));
        }
        Err(error) => return Err(error.to_string()),
    };
    let config: ProjectConfig =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let policy = configured_policy(&config, name, command);
    Ok((config.verification, policy))
}

/// Resolve one deadline, with a per-call override, and bounded resource admission.
pub fn execution_policy(
    root: &Path,
    name: &str,
    command: &str,
    timeout: Option<u64>,
) -> Result<(u64, GateSchedule), String> {
    let (config, policy) = verification_policy(root, name, Some(command))?;
    let timeout = timeout
        .or_else(|| policy.as_ref().and_then(|policy| policy.timeout_secs))
        .unwrap_or(config.timeout_secs);
    if !(1..=magistr_engine::native::CheckOptions::MAX_TIMEOUT_SECS).contains(&timeout) {
        return Err("Check timeout must be between 1 and 3600 seconds.".into());
    }
    if !(1..=16).contains(&config.max_parallel) {
        return Err("Check parallelism must be between 1 and 16.".into());
    }
    Ok((
        timeout,
        GateSchedule {
            max_parallel: config.max_parallel,
            resources: policy.and_then(|policy| policy.resources),
        },
    ))
}
