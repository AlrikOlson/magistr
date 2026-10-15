use crate::cli;
use crate::config::inputs::ProjectInputs;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;
use magistr_engine::native::ObjectiveStatus;
use magistr_engine::{NativeReader, ShipRun};

pub fn cmd_plans(
    dir: &std::path::Path,
    status_filter: Option<cli::PlanStatusFilter>,
    limit: usize,
    renderer: &dyn OutputRenderer,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;
    let native = inputs.workflow().clone().with_project_root(dir);
    let plans = read_plans(&NativeReader::open(&native), status_filter, limit)
        .map_err(error::MagistrError::Workflow)
        .suggest()?;
    if renderer.is_machine_readable() {
        renderer
            .json(
                &mut std::io::stdout(),
                &serde_json::json!({
                    "total": plans.len(), "plans": plans,
                }),
            )
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }
    if plans.is_empty() {
        eprintln!("{}", fmt::dim("No plans found."));
    }
    for plan in plans {
        let status = match plan.objective.status {
            ObjectiveStatus::Defined => "Planned",
            ObjectiveStatus::Active => "In progress",
            ObjectiveStatus::Completed => "Completed",
            ObjectiveStatus::Abandoned => "Abandoned",
        };
        eprintln!(
            "{}  {}  {}",
            fmt::bold(&plan.objective.description),
            status,
            fmt::dim(&plan.run_id)
        );
        for task in plan.tasks {
            eprintln!("  {:?}  {}", task.status, task.title);
        }
    }
    Ok(())
}

fn read_plans(
    reader: &NativeReader,
    status_filter: Option<cli::PlanStatusFilter>,
    limit: usize,
) -> std::result::Result<Vec<ShipRun>, String> {
    let work = reader.ship_snapshot()?;
    let mut plans: Vec<_> = work
        .current
        .into_iter()
        .chain(work.retained.into_iter().rev())
        .filter(|run| match status_filter {
            None => true,
            Some(cli::PlanStatusFilter::Active) => matches!(
                run.objective.status,
                ObjectiveStatus::Defined | ObjectiveStatus::Active
            ),
            Some(cli::PlanStatusFilter::Completed) => {
                run.objective.status == ObjectiveStatus::Completed
            }
            Some(cli::PlanStatusFilter::Abandoned) => {
                run.objective.status == ObjectiveStatus::Abandoned
            }
        })
        .collect();
    plans.sort_by(|a, b| b.objective.created_at.cmp(&a.objective.created_at));
    plans.truncate(limit);
    Ok(plans)
}

#[cfg(test)]
#[path = "cmd_plans.tests.rs"]
mod tests;
