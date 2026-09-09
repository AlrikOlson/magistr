//! `magistr_format` MCP tool — project formatter execution.

use std::path::PathBuf;

use magistr_core::FormatConfig;
use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::{Value, json};
use tracing::debug;

use super::format_runs::FormatRuns;
use super::gate::run_gate_command;

/// The per-file placeholder a format command may carry.
const FILE_PLACEHOLDER: &str = "{file}";

/// Decide the shell command one `magistr_format` call runs.
///
/// - An explicit file list instantiates a `{file}` command once per file
///   (joined with `&&`), or is appended to a command without the placeholder.
/// - No file list (absent or empty) with a `{file}` command expands over the
///   configured extensions with `find | xargs`, pruning the exclusions and
///   `.git` — the placeholder is never run as text. With no extensions there
///   is nothing to expand over, which is reported rather than guessed.
/// - No file list with a whole-project command (`cargo fmt`,
///   `prettier --write .`) runs it exactly as configured.
pub(super) fn resolve_format_command(
    fmt: &FormatConfig,
    files: Option<&[&str]>,
) -> Result<String, String> {
    let has_placeholder = fmt.command.contains(FILE_PLACEHOLDER);
    let files = files.unwrap_or_default();
    if !files.is_empty() {
        let escaped: Vec<String> = files
            .iter()
            .map(|f| shell_escape::escape((*f).into()).into_owned())
            .collect();
        return Ok(if has_placeholder {
            escaped
                .iter()
                .map(|f| fmt.command.replace(FILE_PLACEHOLDER, f))
                .collect::<Vec<_>>()
                .join(" && ")
        } else {
            format!("{} {}", fmt.command, escaped.join(" "))
        });
    }
    if !has_placeholder {
        return Ok(fmt.command.clone());
    }
    if fmt.extensions.is_empty() {
        return Err(format!(
            "format.command `{}` takes one file ({FILE_PLACEHOLDER}) but format.extensions is \
             empty — pass `files`, configure the extensions, or use a whole-project formatter",
            fmt.command
        ));
    }
    let mut exclusions = fmt.exclusions.clone();
    if !exclusions.iter().any(|e| e == ".git") {
        exclusions.push(".git".to_owned());
    }
    Ok(build_find_format_command(
        &fmt.command,
        &fmt.extensions,
        &exclusions,
    ))
}

/// Build a `find` command that enumerates files matching `extensions` while
/// pruning `exclusions` directories, then pipes to `xargs` running `fmt_command`.
///
/// The format command's `{file}` placeholder is replaced by `{}` for xargs.
/// If no `{file}` placeholder exists, files are appended as arguments.
pub(super) fn build_find_format_command(
    fmt_command: &str,
    extensions: &[String],
    exclusions: &[String],
) -> String {
    let mut find = String::from("find . ");

    // Prune excluded directories
    if !exclusions.is_empty() {
        find.push_str("\\( ");
        for (i, dir) in exclusions.iter().enumerate() {
            if i > 0 {
                find.push_str(" -o ");
            }
            let escaped = shell_escape::escape(dir.into());
            find.push_str(&format!("-path './{escaped}'"));
        }
        find.push_str(" \\) -prune -o ");
    }

    // Filter by extensions
    if !extensions.is_empty() {
        find.push_str("\\( ");
        for (i, ext) in extensions.iter().enumerate() {
            if i > 0 {
                find.push_str(" -o ");
            }
            let escaped = shell_escape::escape(ext.into());
            find.push_str(&format!("-name '*.{escaped}'"));
        }
        find.push_str(" \\) ");
    }

    find.push_str("-type f -print0");

    // Build xargs command — replace {file} placeholder or append
    let xargs_cmd = if fmt_command.contains("{file}") {
        let replaced = fmt_command.replace("{file}", "{}");
        format!("xargs -0 -I {{}} {replaced}")
    } else {
        format!("xargs -0 {fmt_command}")
    };

    format!("{find} | {xargs_cmd}")
}

/// Run the formatter for a single archetype, returning a structured result.
async fn run_archetype_format(
    archetype_id: &str,
    fmt: &FormatConfig,
    dir: &std::path::Path,
    files: Option<&[&str]>,
    timeout_secs: u64,
) -> Result<serde_json::Value, magistr_mcp::ToolError> {
    if fmt.command.is_empty() {
        return Ok(json!({
            "archetype": archetype_id,
            "formatted": false,
            "output": "no format command configured",
            "duration_ms": 0,
            "exit_code": 0
        }));
    }
    let command = match resolve_format_command(fmt, files) {
        Ok(cmd) => cmd,
        Err(reason) => {
            return Ok(json!({
                "archetype": archetype_id,
                "formatted": false,
                "command": fmt.command,
                "output": reason,
                "duration_ms": 0,
                "exit_code": 1
            }));
        }
    };
    // The formatter has no pane to stream to; its result is what matters.
    let result = run_gate_command(&command, dir, timeout_secs, |_| {}).await?;
    let passed = result["passed"].as_bool().unwrap_or(false);
    Ok(json!({
        "archetype": archetype_id,
        "formatted": passed,
        "command": result["command"],
        // The directory is part of the command's meaning: a root-relative
        // path in a command run from a sub-directory fails with the tool's
        // own error and no hint that the working directory is why. Reported
        // only on a failure, so a clean format stays the one-line answer it
        // has always been.
        "dir": if passed { Value::Null } else { json!(dir.display().to_string()) },
        "output": result["output"],
        "duration_ms": result["duration_ms"],
        "exit_code": result["exit_code"]
    }))
}

/// Build the `magistr_format` MCP tool for running the project formatter.
///
/// Reads the format command and extensions from `.magistr/project.json`.
/// In polyglot projects, an optional `archetype` parameter targets a specific
/// sub-project; omit it to format all archetypes in parallel.
pub(super) fn build_format_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_format",
        "Run the project formatter. Reads the format command from .magistr/project.json. \
         In polyglot projects, pass 'archetype' to target a specific sub-project; \
         omit it to format all archetypes. An archetype's command runs in the \
         archetype's root, except a command identical to the project-level \
         format.command, which runs at the project root; archetypes sharing a \
         command and a directory are one run. Returns structured results with \
         pass/fail status, output, duration, and — on a failure — the directory \
         the command ran in.",
        json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of file paths to format. If omitted, formats all files matching the configured extensions."
                },
                "archetype": {
                    "type": "string",
                    "description": "Optional archetype ID to target. When omitted in a polyglot project, every archetype is formatted. An archetype's command runs in the archetype's root, EXCEPT a command identical to the project-level format.command, which is the project's own formatter and runs at the project root; archetypes naming the same command in the same directory are one run."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds for the format command. Default: 300.",
                    "default": 300
                }
            }
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                let archetype_filter = input["archetype"].as_str();
                debug!(
                    tool = "magistr_format",
                    archetype = ?archetype_filter,
                    "MCP tool invoked"
                );
                let config = crate::validate::load_project_config(&root).map_err(|e| {
                    magistr_mcp::ToolError::Mcp(format!("Failed to load project config: {e}"))
                })?;
                let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(300);
                let files: Option<Vec<&str>> = input["files"]
                    .as_array()
                    .map(|files| files.iter().filter_map(|f| f.as_str()).collect());

                // ── Archetype-aware path ─────────────────────────────
                if !config.archetypes.is_empty() {
                    if let Some(arch_id) = archetype_filter {
                        let arch = config.find_archetype(arch_id).ok_or_else(|| {
                            let ids: Vec<&str> =
                                config.archetypes.iter().map(|a| a.id.as_str()).collect();
                            magistr_mcp::ToolError::Mcp(format!(
                                "Unknown archetype '{arch_id}'. Available: {}",
                                ids.join(", ")
                            ))
                        })?;
                        // The same rule a whole-project run takes, so
                        // `magistr_format(archetype: "rust")` and
                        // `magistr_format()` cannot disagree about where the
                        // command runs.
                        let plan = FormatRuns::plan(
                            &root,
                            &config.format.command,
                            std::slice::from_ref(arch),
                            files.as_deref(),
                        );
                        let run = &plan.runs()[0];
                        let run_files: Option<Vec<&str>> = run
                            .files
                            .as_ref()
                            .map(|f| f.iter().map(String::as_str).collect());
                        let result = run_archetype_format(
                            arch_id,
                            &run.format,
                            &run.dir,
                            run_files.as_deref(),
                            timeout_secs,
                        )
                        .await?;
                        return Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap_or_default(),
                        ));
                    }

                    // No filter — every archetype, but only the runs they
                    // actually ask for (`FormatRuns`: the project's own
                    // command runs at the project root, and one command in
                    // one directory is one run however many archetypes name
                    // it). Parallel across *distinct* runs only: two
                    // formatters racing over one directory was the bug.
                    let plan = FormatRuns::plan(
                        &root,
                        &config.format.command,
                        &config.archetypes,
                        files.as_deref(),
                    );
                    let mut handles = Vec::new();
                    for run in plan.runs() {
                        if run.is_empty() {
                            handles.push(None);
                            continue;
                        }
                        let ids = run.archetypes.clone();
                        let fmt = run.format.clone();
                        let dir = run.dir.clone();
                        let run_files: Option<Vec<String>> = run.files.clone();
                        handles.push(Some(tokio::spawn(async move {
                            let refs: Option<Vec<&str>> = run_files
                                .as_ref()
                                .map(|f| f.iter().map(String::as_str).collect());
                            run_archetype_format(
                                &ids.join(", "),
                                &fmt,
                                &dir,
                                refs.as_deref(),
                                timeout_secs,
                            )
                            .await
                        })));
                    }
                    let mut arch_results = Vec::new();
                    for (run, handle) in plan.runs().iter().zip(handles) {
                        let outcome = match handle {
                            Some(handle) => handle.await.map_err(|e| {
                                magistr_mcp::ToolError::Mcp(format!("format task panicked: {e}"))
                            })??,
                            // Nothing of this archetype's was named, so
                            // nothing ran — said plainly rather than reported
                            // as a formatter that did nothing.
                            None => json!({
                                "formatted": true,
                                "output": "no named file belongs to this archetype",
                                "duration_ms": 0,
                                "exit_code": 0
                            }),
                        };
                        // One entry per archetype, as the response has always
                        // carried: archetypes sharing a run share its outcome.
                        for id in &run.archetypes {
                            let mut entry = outcome.clone();
                            entry["archetype"] = json!(id);
                            arch_results.push(entry);
                        }
                    }
                    let all_formatted = arch_results
                        .iter()
                        .all(|r| r["formatted"].as_bool().unwrap_or(false));
                    let response = json!({
                        "formatted": all_formatted,
                        "archetypes": arch_results
                    });
                    return Ok(ToolResult::text(
                        serde_json::to_string_pretty(&response).unwrap_or_default(),
                    ));
                }

                // ── Legacy single-archetype path ─────────────────────
                let fmt = &config.format;

                if fmt.command.is_empty() {
                    return Ok(ToolResult::text(
                        serde_json::to_string_pretty(&json!({
                            "formatted": false,
                            "output": "no format command configured",
                            "duration_ms": 0,
                            "exit_code": 0
                        }))
                        .unwrap_or_default(),
                    ));
                }

                let command = match resolve_format_command(fmt, files.as_deref()) {
                    Ok(command) => command,
                    Err(reason) => {
                        return Ok(ToolResult::text(
                            serde_json::to_string_pretty(&json!({
                                "formatted": false,
                                "command": fmt.command,
                                "output": reason,
                                "duration_ms": 0,
                                "exit_code": 1
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                };

                let result = run_gate_command(&command, &root, timeout_secs, |_| {}).await?;
                let passed = result["passed"].as_bool().unwrap_or(false);

                // Re-wrap as format-specific response
                Ok(ToolResult::text(
                    serde_json::to_string_pretty(&json!({
                        "formatted": passed,
                        "command": result["command"],
                        "output": result["output"],
                        "duration_ms": result["duration_ms"],
                        "exit_code": result["exit_code"]
                    }))
                    .unwrap_or_default(),
                ))
            })
        },
    )
}
