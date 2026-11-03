//! `magistr_check` MCP tool — check execution.

use std::path::{Path, PathBuf};

use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

// The output budget and the truncation live with the shared check runner
// (`crate::utils::check`), which parses failure rows from the full output
// before applying them.

/// Run a check command with timeout, graceful process cleanup, and stdio
/// isolation.
///
/// Delegates to the shared check runner ([`crate::utils::check::run_check`]),
/// which runs the command on a pty, hands every task it writes to `sink`
/// while it runs, and handles process-group management, SIGTERM→SIGKILL
/// escalation and orphan prevention. Returns structured JSON with
/// pass/fail, the stripped transcript, duration, and exit code.
pub(super) async fn run_check_command(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    sink: impl FnMut(&[u8]) + Send,
) -> Result<serde_json::Value, magistr_mcp::ToolError> {
    let run = crate::utils::check::run_check(
        "",
        command,
        true,
        dir,
        timeout_secs,
        &tokio_util::sync::CancellationToken::new(),
        sink,
    )
    .await;
    Ok(check_json(&run))
}

/// The JSON view of a check run — the shape `magistr_check` has always
/// answered with, plus the `verdict` that tells a timeout apart from a
/// failure. The callers overwrite `check` and `required` with the spec's.
pub(super) fn check_json(run: &crate::utils::check::CheckRun) -> serde_json::Value {
    json!({
        "passed": run.passed(),
        "verdict": run.verdict.label(),
        "output": run.output,
        "rows": run.rows,
        "duration_ms": run.duration.as_millis() as u64,
        "queue_ms": run.queue_duration.as_millis() as u64,
        "execution_ms": run.duration.saturating_sub(run.queue_duration).as_millis() as u64,
        "exit_code": run.exit_code,
        "command": run.command
    })
}

// One resolver for every consumer — this tool, the mcp service,
// project validation and the verification evidence a task must present —
// lives in magistr-core, so none of them can disagree about what a check
// runs.
pub(crate) use magistr_core::checks::{CheckSpec, resolve_checks};

/// Build the `magistr_check` MCP tool for running checks.
///
/// Reads check commands from `.magistr/project.json` and executes them with
/// structured JSON results including exit code, duration, and output.
///
/// When the project defines archetypes, an optional `archetype` parameter
/// selects a specific sub-project. When omitted, checks run for all archetypes
/// with bounded concurrency.
pub(super) fn build_check_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_check",
        "Run check commands. Standard checks: check, test, lint, all. \
         Custom checks defined in project.json custom_checks array can also be \
         run by name. 'all' runs standard + required custom checks. \
         In polyglot projects, pass 'archetype' to target a specific sub-project; \
         omit it to run checks for all archetypes. Returns structured results \
         with pass/fail status, output, duration, and exit code. Each check is \
         recorded through check_step (verified, real exit code) when a run task \
         is active in this server; otherwise the check still runs and `recorded: \
         false` + `note` say why it was not recorded (`all` carries both at the \
         top level too).",
        json!({
            "type": "object",
            "properties": {
                "check": {
                    "type": "string",
                    "description": "Which check to run: 'check', 'test', 'lint', 'all', or a custom check name."
                },
                "archetype": {
                    "type": "string",
                    "description": "Optional archetype ID to target. When set, runs only that archetype's checks from its root path. When omitted in a polyglot project, runs checks for all archetypes."
                },
                "diagnostic": {"type":"boolean","default":false,"description":"Collect all selected failures instead of stopping after the first required failure."},
                "reuse": {"type":"boolean","default":true,"description":"Reuse matching verified evidence; false forces execution."},
                "inputs": {"type":"array","items":{"type":"string"},"description":"Relative source files/directories including dependencies; omitted observes the whole source tree."},
                "timeout_secs": {
                    "type": "integer",
                    "description": "Override the configured per-check deadline (1–3600 seconds).",
                    "minimum": 1,
                    "maximum": 3600
                }
            },
            "required": ["check"]
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                let check_name = input["check"].as_str().ok_or_else(|| {
                    magistr_mcp::ToolError::Mcp("missing 'check' parameter".into())
                })?;
                let archetype_filter = input["archetype"].as_str();
                let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(0);
                debug!(
                    tool = "magistr_check",
                    check = check_name,
                    archetype = ?archetype_filter,
                    timeout_secs,
                    "MCP tool invoked"
                );

                // Load project config to get check commands
                let config = crate::validate::load_project_config(&root).map_err(|e| {
                    magistr_mcp::ToolError::Mcp(format!("Failed to load project config: {e}"))
                })?;

                // One resolver for every path: the specs already carry the
                // polyglot rules (per-stack coverage, dedup, cd-prefixed
                // subdirectory commands), so this tool and the mcp
                // service cannot disagree about what a check runs.
                let cfg = match archetype_filter {
                    Some(arch_id) => {
                        if config.find_archetype(arch_id).is_none() {
                            let ids: Vec<&str> =
                                config.archetypes.iter().map(|a| a.id.as_str()).collect();
                            return Ok(ToolResult::error(format!(
                                "Unknown archetype '{arch_id}'. Available: {}",
                                ids.join(", ")
                            )));
                        }
                        // Narrow to the one archetype; custom checks are
                        // project-level and stay out of a targeted run.
                        let mut narrowed = config.clone();
                        narrowed.archetypes.retain(|a| a.id == arch_id);
                        narrowed.custom_checks.clear();
                        narrowed
                    }
                    None => config,
                };
                let specs = match resolve_checks(&cfg, check_name) {
                    Ok(specs) => specs,
                    Err(message) => return Ok(ToolResult::error(message)),
                };

                let diagnostic = input["diagnostic"].as_bool().unwrap_or(false);
                let mut results = super::check_batch::run_batch(
                    specs,
                    cfg.verification.max_parallel,
                    diagnostic,
                    |spec| {
                        let dir = root.clone();
                        async move {
                            let run = crate::utils::check::run_check(
                                &spec.name,
                                &spec.command,
                                spec.required,
                                &dir,
                                timeout_secs,
                                &tokio_util::sync::CancellationToken::new(),
                                |_| {},
                            )
                            .await;
                            let mut result = check_json(&run);
                            result["check"] = json!(spec.name);
                            result["required"] = json!(spec.required);
                            result["recorded"] = json!(false);
                            result["note"] =
                                json!("No workflow owner is attached to this standalone tool.");
                            result
                        }
                    },
                )
                .await;

                // A single-spec check answers with the bare record; anything
                // multi-spec (a polyglot standard check, or `all`) answers
                // with the grouped shape.
                let body = if check_name != "all" && results.len() == 1 {
                    results.pop().unwrap_or_else(|| json!({ "passed": false }))
                } else {
                    let all_passed = results.iter().all(|r| {
                        let passed = r["passed"].as_bool().unwrap_or(false);
                        let required = r["required"].as_bool().unwrap_or(true);
                        passed || !required
                    });
                    json!({ "passed": all_passed, "checks": results })
                };
                Ok(ToolResult::text(
                    serde_json::to_string_pretty(&body).unwrap_or_default(),
                ))
            })
        },
    )
}

#[cfg(test)]
#[path = "check.tests.rs"]
mod tests;
