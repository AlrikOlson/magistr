use crate::cli;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use crate::workflow;
use color_eyre::eyre;

pub fn cmd_plans(
    dir: &std::path::Path,
    status_filter: Option<cli::PlanStatusFilter>,
    limit: usize,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let mut plans = workflow::plans::list_plans(&dir).suggest()?;

    // Apply status filter
    if let Some(filter) = status_filter {
        let target = match filter {
            cli::PlanStatusFilter::Active => workflow::plans::PlanStatus::Active,
            cli::PlanStatusFilter::Completed => workflow::plans::PlanStatus::Completed,
            cli::PlanStatusFilter::Abandoned => workflow::plans::PlanStatus::Abandoned,
        };
        plans.retain(|p| p.status == target);
    }

    plans.truncate(limit);

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "plans": plans.iter().map(|p| serde_json::json!({
                "id": p.id,
                "status": p.status.label(),
                "task_name": p.task_name,
                "created_at": p.created_at.to_rfc3339(),
                "spec_items": p.spec_items,
            })).collect::<Vec<_>>(),
            "total": plans.len(),
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    if plans.is_empty() {
        eprintln!("{}", fmt::dim("No plans found."));
        return Ok(());
    }

    eprintln!("{}\n", fmt::bold("📋 Plans"));

    for plan in &plans {
        let ts = plan.created_at.format("%Y-%m-%d %H:%M");
        let (icon_str, name_str) = match plan.status {
            workflow::plans::PlanStatus::Active => (fmt::green("●"), fmt::green(&plan.task_name)),
            workflow::plans::PlanStatus::Completed => (fmt::cyan("✓"), fmt::cyan(&plan.task_name)),
            workflow::plans::PlanStatus::Abandoned => (fmt::dim("✗"), fmt::dim(&plan.task_name)),
        };
        eprintln!(
            "  {icon_str}  {}  {name_str}  {}",
            fmt::dim(&ts.to_string()),
            fmt::dim(&plan.id)
        );
    }

    eprintln!();
    Ok(())
}
