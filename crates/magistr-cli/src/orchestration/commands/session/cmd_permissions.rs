use std::io::Write;

use crate::cli::ExportFormat;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::permissions;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_permissions(
    limit: usize,
    project: Option<&str>,
    actor: Option<&str>,
    export: Option<ExportFormat>,
    renderer: &dyn OutputRenderer,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let path = permissions::audit::audit_log_path(inputs).ok_or_else(|| {
        error::MagistrError::Config("could not determine data directory (HOME not set)".into())
    })?;

    let mut entries = permissions::audit::load_entries(&path).suggest()?;

    // Filter by project if specified
    if let Some(proj) = project {
        entries.retain(|e| e.project.as_deref().is_some_and(|p| p.contains(proj)));
    }

    // Filter by actor if specified
    if let Some(who) = actor {
        entries.retain(|e| e.actor.as_deref().is_some_and(|a| a.contains(who)));
    }

    // Export leaves the machine: every matching entry, oldest first,
    // untouched by the display limit.
    if let Some(format) = export {
        entries.sort_by_key(|e| e.timestamp);
        let out = match format {
            ExportFormat::Json => {
                let mut json = serde_json::to_string_pretty(&entries)
                    .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
                json.push('\n');
                json
            }
            ExportFormat::Csv => permissions::audit::export_csv(&entries),
        };
        std::io::stdout()
            .write_all(out.as_bytes())
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    // Sort by timestamp descending (most recent first)
    entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
    entries.truncate(limit);

    if renderer.is_machine_readable() {
        let summary = permissions::audit::summarize(&entries);
        let data = serde_json::json!({
            "entries": entries,
            "summary": {
                "total": summary.total,
                "allowed": summary.allowed,
                "denied": summary.denied,
                "top_tools": summary.top_allowed_tools,
            },
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    if entries.is_empty() {
        eprintln!("{}", fmt::dim("No permission audit entries found."));
        return Ok(());
    }

    let summary = permissions::audit::summarize(&entries);

    eprintln!("{}\n", fmt::bold("🔐 Permission Audit Log"));

    for entry in &entries {
        let ts = entry.timestamp.format("%Y-%m-%d %H:%M:%S");
        let status = if entry.allowed {
            fmt::icon_pass()
        } else {
            fmt::icon_fail()
        };
        let risk_badge = entry.risk.badge();
        let project_str = entry.project.as_deref().unwrap_or("unknown");
        let actor_str = entry.actor.as_deref().unwrap_or("-");

        eprintln!(
            "  {}  {status}  {risk_badge} {}  {}  {}  {}",
            fmt::dim(&ts.to_string()),
            fmt::bold(&entry.tool_name),
            fmt::dim(&entry.reason),
            fmt::dim(project_str),
            fmt::dim(actor_str),
        );
    }

    eprintln!();
    eprintln!(
        "  {} {} entries — {} allowed, {} denied",
        fmt::bold("Total:"),
        summary.total,
        summary.allowed,
        summary.denied,
    );

    if let Some(suggestion) = permissions::audit::suggest_trust_upgrade(&entries) {
        eprintln!();
        eprintln!("  {}", fmt::yellow(&format!("💡 {suggestion}")));
    }

    Ok(())
}
