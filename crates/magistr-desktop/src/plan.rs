//! Desktop shape of the shared plan policy, derived from one complete read.
use magistr_workflow_contracts::plan::{
    WorkstreamCatalog, WorkstreamFrontier, workstream_catalog, workstream_frontier,
};
use magistr_workflow_contracts::{ReadState, PlanData};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Serialize)]
pub(crate) struct PlanOverview {
    catalog: WorkstreamCatalog,
    frontiers: Vec<WorkstreamFrontier>,
}

pub(crate) fn overview(read: &ReadState<PlanData>) -> ReadState<PlanOverview> {
    read.map(|data| {
        let catalog = workstream_catalog(&data.tasks);
        let features: BTreeSet<_> = catalog
            .current
            .keys()
            .chain(catalog.history.keys())
            .collect();
        let mut frontiers: Vec<_> = features
            .into_iter()
            .map(|feature| workstream_frontier(&data.tasks, Some(feature)))
            .collect();
        if catalog.unassigned.counts.total > 0
            || catalog.unassigned_history.done + catalog.unassigned_history.retired > 0
        {
            frontiers.push(workstream_frontier(&data.tasks, None));
        }
        PlanOverview { catalog, frontiers }
    })
}

#[cfg(test)]
#[path = "plan.tests.rs"]
mod tests;
