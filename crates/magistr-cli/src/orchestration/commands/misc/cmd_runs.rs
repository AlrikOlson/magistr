use crate::cli;
use crate::config::inputs::ProjectInputs;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;
use magistr_engine::native::GoalStatus;
use magistr_engine::{NativeReader, Run};

pub fn cmd_runs(
    dir: &std::path::Path,
    status_filter: Option<cli::RunStatusFilter>,
    limit: usize,
    renderer: &dyn OutputRenderer,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;
    let native = inputs.workflow().clone().with_project_root(dir);
    let runs = read_runs(&NativeReader::open(&native), status_filter, limit)
        .map_err(error::MagistrError::Workflow)
        .suggest()?;
    if renderer.is_machine_readable() {
        renderer
            .json(
                &mut std::io::stdout(),
                &serde_json::json!({
                    "total": runs.len(), "runs": runs,
                }),
            )
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }
    if runs.is_empty() {
        eprintln!("{}", fmt::dim("No runs found."));
    }
    for run in runs {
        let status = match run.goal.status {
            GoalStatus::Open => "Open",
            GoalStatus::Active => "Active",
            GoalStatus::Finished => "Finished",
            GoalStatus::Abandoned => "Abandoned",
        };
        eprintln!(
            "{}  {}  {}",
            fmt::bold(&run.goal.description),
            status,
            fmt::dim(&run.run_id)
        );
        for step in run.steps {
            eprintln!("  {:?}  {}", step.status, step.title);
        }
    }
    Ok(())
}

fn read_runs(
    reader: &NativeReader,
    status_filter: Option<cli::RunStatusFilter>,
    limit: usize,
) -> std::result::Result<Vec<Run>, String> {
    let work = reader.run_snapshot()?;
    let mut runs: Vec<_> = work
        .current
        .into_iter()
        .chain(work.retained.into_iter().rev())
        .filter(|run| match status_filter {
            None => true,
            Some(cli::RunStatusFilter::Open) => run.goal.status == GoalStatus::Open,
            Some(cli::RunStatusFilter::Active) => run.goal.status == GoalStatus::Active,
            Some(cli::RunStatusFilter::Finished) => run.goal.status == GoalStatus::Finished,
            Some(cli::RunStatusFilter::Abandoned) => run.goal.status == GoalStatus::Abandoned,
        })
        .collect();
    runs.sort_by(|a, b| b.goal.created_at.cmp(&a.goal.created_at));
    runs.truncate(limit);
    Ok(runs)
}

#[cfg(test)]
#[path = "cmd_runs.tests.rs"]
mod tests;
