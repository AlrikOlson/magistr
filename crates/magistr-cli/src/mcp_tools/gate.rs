//! `magistr_gate` MCP tool — quality gate execution.

use std::path::{Path, PathBuf};

use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

// The output budget and the truncation live with the shared gate runner
// (`crate::utils::gate`), which parses failure rows from the full output
// before applying them.

/// Run a gate command with timeout, graceful process cleanup, and stdio
/// isolation.
///
/// Delegates to the shared gate runner ([`crate::utils::gate::run_gate`]),
/// which runs the command on a pty, hands every chunk it writes to `sink`
/// while it runs, and handles process-group management, SIGTERM→SIGKILL
/// escalation and orphan prevention. Returns structured JSON with
/// pass/fail, the stripped transcript, duration, and exit code.
pub(super) async fn run_gate_command(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    sink: impl FnMut(&[u8]) + Send,
) -> Result<serde_json::Value, magistr_mcp::ToolError> {
    let run = crate::utils::gate::run_gate(
        "",
        command,
        true,
        dir,
        timeout_secs,
        &tokio_util::sync::CancellationToken::new(),
        sink,
    )
    .await;
    Ok(gate_json(&run))
}

/// The JSON view of a gate run — the shape `magistr_gate` has always
/// answered with, plus the `verdict` that tells a timeout apart from a
/// failure. The callers overwrite `gate` and `required` with the spec's.
pub(super) fn gate_json(run: &crate::utils::gate::GateRun) -> serde_json::Value {
    json!({
        "passed": run.passed(),
        "verdict": run.verdict.label(),
        "output": run.output,
        "rows": run.rows,
        "duration_ms": run.duration.as_millis() as u64,
        "exit_code": run.exit_code,
        "command": run.command
    })
}

/// One gate to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GateSpec {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) required: bool,
}

/// Resolve `gate` (`check` | `test` | `lint` | `all` | a custom gate name)
/// against the project config. `Err` carries the user-facing message that
/// names the valid gates.
///
/// On a polyglot project the archetypes are the truth: each standard gate
/// resolves to one spec per distinct (command, directory) across the
/// archetypes — so every stack's gate runs, and a workspace-wide command
/// named by eight archetypes runs once instead of being raced against
/// itself. A project without archetypes resolves to the top-level commands
/// exactly as before.
pub(crate) fn resolve_gates(
    config: &crate::types::ProjectConfig,
    gate: &str,
) -> Result<Vec<GateSpec>, String> {
    match gate {
        "check" | "test" | "lint" => Ok(standard_specs(config, gate)),
        "all" => {
            let mut specs = Vec::new();
            for g in ["check", "test", "lint"] {
                specs.extend(standard_specs(config, g));
            }
            // Standard + *required* custom gates, as this tool's own
            // description has always claimed. A gate registered
            // `required: false` is one the project expects to fail; `all`
            // paying for it bought a wasted run and a FAIL row that read
            // like a failed iteration. It stays runnable by its own name.
            specs.extend(
                config
                    .custom_gates
                    .iter()
                    .filter(|cg| cg.required)
                    .map(|cg| GateSpec {
                        name: cg.name.clone(),
                        command: cg.command.clone(),
                        required: cg.required,
                    }),
            );
            Ok(specs)
        }
        other => config
            .custom_gates
            .iter()
            .find(|cg| cg.name == other)
            .map(|cg| {
                vec![GateSpec {
                    name: cg.name.clone(),
                    command: cg.command.clone(),
                    required: cg.required,
                }]
            })
            .ok_or_else(|| {
                let mut valid: Vec<&str> = vec!["check", "test", "lint", "all"];
                valid.extend(config.custom_gates.iter().map(|cg| cg.name.as_str()));
                format!("Unknown gate '{other}'. Valid gates: {}", valid.join(", "))
            }),
    }
}

/// The specs for one standard gate (`check` | `test` | `lint`).
///
/// Without archetypes: the top-level command, named after the gate. With
/// archetypes, three rules decide, all shared with `magistr_format`'s
/// format-archetype-cwd fix: (1) an archetype naming the project's own
/// top-level command is running the *project's* gate and runs at the project
/// root, whichever subdirectory the archetype lives in; (2) one command in
/// one directory is one run, however many archetypes name it; (3) a genuinely
/// different command in a subdirectory archetype runs in that directory —
/// spelled `cd <dir> && <cmd>` so a record of the command is a record of
/// where it ran. Specs carrying an archetype's own command are named
/// `{gate}:{archetype id}` so their check records stay distinguishable.
fn standard_specs(config: &crate::types::ProjectConfig, gate: &str) -> Vec<GateSpec> {
    let qg = &config.quality_gate;
    let top = match gate {
        "check" => &qg.check,
        "test" => &qg.test,
        _ => &qg.lint,
    };
    let top_spec = || GateSpec {
        name: gate.to_owned(),
        command: top.clone(),
        required: true,
    };
    if config.archetypes.is_empty() {
        return vec![top_spec()];
    }

    let mut specs: Vec<GateSpec> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for arch in &config.archetypes {
        let raw = match gate {
            "check" => &arch.quality_gate.check,
            "test" => &arch.quality_gate.test,
            _ => &arch.quality_gate.lint,
        };
        if raw.is_empty() {
            continue;
        }
        let root = arch.root.to_string_lossy();
        let is_project_command = raw == top;
        let dir = if is_project_command || root.is_empty() || root == "." {
            String::new()
        } else {
            root.to_string()
        };
        if !seen.insert((raw.clone(), dir.clone())) {
            continue;
        }
        let name = if is_project_command {
            gate.to_owned()
        } else {
            format!("{gate}:{}", arch.id)
        };
        let command = if dir.is_empty() {
            raw.clone()
        } else {
            format!("cd {} && {raw}", shell_escape::escape(dir.into()))
        };
        specs.push(GateSpec {
            name,
            command,
            required: true,
        });
    }
    if specs.is_empty() {
        // Every archetype left this gate blank — the top-level command is
        // still the project's answer.
        return vec![top_spec()];
    }
    specs
}

/// Build the `magistr_gate` MCP tool for running quality gates.
///
/// Reads gate commands from `.magistr/project.json` and executes them with
/// structured JSON results including exit code, duration, and output.
///
/// When the project defines archetypes, an optional `archetype` parameter
/// selects a specific sub-project. When omitted, gates run for all archetypes
/// in parallel.
pub(super) fn build_gate_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_gate",
        "Run quality gate commands. Standard gates: check, test, lint, all. \
         Custom gates defined in project.json custom_gates array can also be \
         run by name. 'all' runs standard + required custom gates. \
         In polyglot projects, pass 'archetype' to target a specific sub-project; \
         omit it to run gates for all archetypes. Returns structured results \
         with pass/fail status, output, duration, and exit code. Each gate is \
         recorded through ship_check (verified, real exit code) when a ship task \
         is active in this server; otherwise the gate still runs and `recorded: \
         false` + `note` say why it was not recorded (`all` carries both at the \
         top level too).",
        json!({
            "type": "object",
            "properties": {
                "gate": {
                    "type": "string",
                    "description": "Which gate to run: 'check', 'test', 'lint', 'all', or a custom gate name."
                },
                "archetype": {
                    "type": "string",
                    "description": "Optional archetype ID to target. When set, runs only that archetype's gates from its root path. When omitted in a polyglot project, runs gates for all archetypes."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds for each gate command. Default: 300.",
                    "default": 300
                }
            },
            "required": ["gate"]
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                let gate_name = input["gate"].as_str().ok_or_else(|| {
                    magistr_mcp::ToolError::Mcp("missing 'gate' parameter".into())
                })?;
                let archetype_filter = input["archetype"].as_str();
                let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(300);
                debug!(
                    tool = "magistr_gate",
                    gate = gate_name,
                    archetype = ?archetype_filter,
                    timeout_secs,
                    "MCP tool invoked"
                );

                // Load project config to get gate commands
                let config = crate::validate::load_project_config(&root).map_err(|e| {
                    magistr_mcp::ToolError::Mcp(format!("Failed to load project config: {e}"))
                })?;

                // One resolver for every path: the specs already carry the
                // polyglot rules (per-stack coverage, dedup, cd-prefixed
                // subdirectory commands), so this tool and the mcp-serve
                // service cannot disagree about what a gate runs.
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
                        // Narrow to the one archetype; custom gates are
                        // project-level and stay out of a targeted run.
                        let mut narrowed = config.clone();
                        narrowed.archetypes.retain(|a| a.id == arch_id);
                        narrowed.custom_gates.clear();
                        narrowed
                    }
                    None => config,
                };
                let specs = match resolve_gates(&cfg, gate_name) {
                    Ok(specs) => specs,
                    Err(message) => return Ok(ToolResult::error(message)),
                };

                let mut handles = Vec::new();
                for spec in &specs {
                    let spec = spec.clone();
                    let dir = root.clone();
                    handles.push(tokio::spawn(async move {
                        // The standalone tool has no feed socket to stream
                        // on; `magistr mcp-serve`'s service is the caller
                        // that broadcasts a gate's bytes.
                        let mut result =
                            run_gate_command(&spec.command, &dir, timeout_secs, |_| {}).await?;
                        result["gate"] = json!(spec.name);
                        result["required"] = json!(spec.required);
                        Ok::<_, magistr_mcp::ToolError>(result)
                    }));
                }
                let mut results = Vec::with_capacity(handles.len());
                for handle in handles {
                    let result = handle.await.map_err(|e| {
                        magistr_mcp::ToolError::Mcp(format!("gate task panicked: {e}"))
                    })??;
                    results.push(result);
                }

                // A single-spec gate answers with the bare record; anything
                // multi-spec (a polyglot standard gate, or `all`) answers
                // with the grouped shape.
                let body = if gate_name != "all" && results.len() == 1 {
                    results.pop().unwrap_or_else(|| json!({ "passed": false }))
                } else {
                    let all_passed = results.iter().all(|r| {
                        let passed = r["passed"].as_bool().unwrap_or(false);
                        let required = r["required"].as_bool().unwrap_or(true);
                        passed || !required
                    });
                    json!({ "passed": all_passed, "gates": results })
                };
                Ok(ToolResult::text(
                    serde_json::to_string_pretty(&body).unwrap_or_default(),
                ))
            })
        },
    )
}

#[cfg(test)]
#[path = "gate.tests.rs"]
mod tests;
