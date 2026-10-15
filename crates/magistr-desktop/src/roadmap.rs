//! Desktop shape of the shared roadmap policy, derived from one complete read.
use magistr_workflow_contracts::roadmap::{
    WorkstreamCatalog, WorkstreamFrontier, workstream_catalog, workstream_frontier,
};
use magistr_workflow_contracts::{ReadState, RoadmapData};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Serialize)]
pub(crate) struct RoadmapOverview {
    catalog: WorkstreamCatalog,
    frontiers: Vec<WorkstreamFrontier>,
}

pub(crate) fn overview(read: &ReadState<RoadmapData>) -> ReadState<RoadmapOverview> {
    read.map(|data| {
        let catalog = workstream_catalog(&data.chunks);
        let groups: BTreeSet<_> = catalog
            .current
            .keys()
            .chain(catalog.history.keys())
            .collect();
        let mut frontiers: Vec<_> = groups
            .into_iter()
            .map(|group| workstream_frontier(&data.chunks, Some(group)))
            .collect();
        if catalog.ungrouped.counts.total > 0
            || catalog.ungrouped_history.done + catalog.ungrouped_history.obsoleted > 0
        {
            frontiers.push(workstream_frontier(&data.chunks, None));
        }
        RoadmapOverview { catalog, frontiers }
    })
}

#[cfg(test)]
#[path = "roadmap.tests.rs"]
mod tests;
