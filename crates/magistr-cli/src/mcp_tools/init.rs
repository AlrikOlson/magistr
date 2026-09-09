//! `magistr_init` MCP tool — programmatic project initialization.

use std::path::PathBuf;

use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

/// Build the `magistr_init` MCP tool for programmatic project initialization.
pub(super) fn build_init_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_init",
        "Initialize or re-initialize a project for magistr. \
         Detects the project stack (single or polyglot), scaffolds .claude/ config, \
         and returns structured results. In polyglot repos, generates multi-archetype \
         project.json with per-archetype commands. \
         Use 'check' mode to compare filesystem state against existing config \
         without making changes.",
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "description": "Init mode: 'init' (default) scaffolds config, 'check' reports drift without changes, 'dry_run' previews what would be created.",
                    "enum": ["init", "check", "dry_run"],
                    "default": "init"
                },
                "force": {
                    "type": "boolean",
                    "description": "Allow re-initialization of an existing project; refresh generated files while preserving project configuration, customizations and workflow state.",
                    "default": false
                }
            }
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                let mode = input.get("mode").and_then(|v| v.as_str()).unwrap_or("init");
                let force = input
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                debug!(tool = "magistr_init", mode, force, "MCP tool invoked");

                match mode {
                    "check" => run_check(&root),
                    "dry_run" => run_dry_run(&root),
                    "init" => run_init(&root, force).await,
                    _ => Ok(ToolResult::error(format!(
                        "Unknown mode '{mode}'. Valid: init, check, dry_run"
                    ))),
                }
            })
        },
    )
}

/// Run drift detection and return structured results.
fn run_check(root: &std::path::Path) -> Result<ToolResult, magistr_mcp::ToolError> {
    let config = crate::validate::load_project_config(root).map_err(|e| {
        magistr_mcp::ToolError::Mcp(format!(
            "Cannot load project config: {e}. Run `magistr init` first."
        ))
    })?;
    let archetypes = config.effective_archetypes();

    let tree = crate::detect::build_project_tree(root)
        .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Detection failed: {e}")))?;

    let report = crate::scaffold::drift::detect_drift(&archetypes, &tree);

    let new_projects: Vec<serde_json::Value> = report
        .new_projects
        .iter()
        .map(|p| {
            json!({
                "path": p.path.display().to_string(),
                "language": p.language.as_str(),
                "framework": p.framework.map(|f| f.as_str()),
            })
        })
        .collect();

    let changes: Vec<serde_json::Value> = report
        .changes
        .iter()
        .map(|c| {
            json!({
                "archetype": c.archetype_id,
                "description": c.description,
            })
        })
        .collect();

    let result = json!({
        "clean": report.is_clean(),
        "total_changes": report.total(),
        "new_projects": new_projects,
        "removed_archetypes": report.removed_archetypes,
        "changes": changes,
    });

    Ok(ToolResult::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    ))
}

/// Run a dry-run preview and return structured results.
fn run_dry_run(root: &std::path::Path) -> Result<ToolResult, magistr_mcp::ToolError> {
    let stack = crate::detect::detect_stack(root)
        .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Detection failed: {e}")))?;

    let pipeline = crate::scaffold::pipeline::ScaffoldPipeline::headless(stack);
    let preview = pipeline
        .preview(root)
        .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Preview failed: {e}")))?;

    let files: Vec<serde_json::Value> = preview
        .files
        .iter()
        .map(|f| {
            json!({
                "path": f.path,
                "action": format!("{:?}", f.action).to_lowercase(),
            })
        })
        .collect();

    let result = json!({
        "mode": "dry_run",
        "creates": preview.creates(),
        "overwrites": preview.overwrites(),
        "unchanged": preview.unchanged(),
        "preserved": preview.preserved(),
        "files": files,
    });

    Ok(ToolResult::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    ))
}

/// Run the full init and return structured results.
async fn run_init(
    root: &std::path::Path,
    force: bool,
) -> Result<ToolResult, magistr_mcp::ToolError> {
    use crate::commands;

    if !force {
        let claude_dir = root.join(".claude");
        if claude_dir.exists() {
            return Ok(ToolResult::error(
                "Project already initialized. Pass force: true to re-initialize, \
                 or use mode: 'check' to see drift."
                    .to_string(),
            ));
        }
    }
    commands::misc::ensure_no_existing_scaffold(root, force)
        .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Scaffold preflight failed: {e}")))?;

    // Try tree detection for polyglot
    let tree = crate::detect::build_project_tree(root)
        .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Detection failed: {e}")))?;

    let (written, is_polyglot, archetype_count) = if tree.len() > 1 {
        let pipeline = crate::scaffold::pipeline::ScaffoldPipeline::polyglot(&tree);
        let written = pipeline
            .generate(root)
            .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Scaffold failed: {e}")))?;
        (written, true, tree.len())
    } else {
        let stack = crate::detect::detect_stack(root)
            .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Detection failed: {e}")))?;
        let pipeline = crate::scaffold::pipeline::ScaffoldPipeline::headless(stack);
        let written = pipeline
            .generate(root)
            .map_err(|e| magistr_mcp::ToolError::Mcp(format!("Scaffold failed: {e}")))?;
        (written, false, 1)
    };

    let result = json!({
        "mode": "init",
        "polyglot": is_polyglot,
        "archetype_count": archetype_count,
        "files_written": written.len(),
        "files": written,
    });

    Ok(ToolResult::text(
        serde_json::to_string_pretty(&result).unwrap_or_default(),
    ))
}

#[cfg(test)]
#[path = "init.tests.rs"]
mod tests;
