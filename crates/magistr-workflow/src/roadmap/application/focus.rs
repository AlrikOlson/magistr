use crate::roadmap::domain::{
    FocusMode, FocusState, WorkstreamCatalog, WorkstreamResolution, validate_lane,
};
use crate::roadmap::engine::RoadmapEngine;
use magistr_workflow_contracts::Observed;

use super::{
    FocusChange, FocusRead, GroupProposal, RoadmapError, RoadmapUseCases, WorkstreamInspection,
    WorkstreamView,
};

impl RoadmapUseCases {
    pub fn focus_get(
        &self,
        lane: &str,
        group: Option<&str>,
        view: WorkstreamView,
    ) -> Result<FocusRead, RoadmapError> {
        let engine = self.lock()?;
        let workstreams = engine.workstreams();
        let focus = engine.focus_get(lane).cloned();
        let focus_state = workstreams.focus_state(focus.as_ref().map(|focus| focus.group.as_str()));
        let frontier = match group.map(str::trim).filter(|group| !group.is_empty()) {
            Some(query) => match workstreams.resolve_current(query) {
                WorkstreamResolution::Current { group }
                | WorkstreamResolution::Closed { group, .. } => inspect_exact(
                    &engine,
                    &workstreams,
                    &workstreams.focus_state(Some(&group)),
                    view,
                ),
                WorkstreamResolution::Unknown { .. } => WorkstreamInspection::Missing {
                    query: query.to_owned(),
                },
                WorkstreamResolution::Ambiguous { matches } => WorkstreamInspection::Ambiguous {
                    query: query.to_owned(),
                    matches,
                },
            },
            None => inspect_exact(&engine, &workstreams, &focus_state, view),
        };
        Ok(FocusRead {
            lane: lane.to_owned(),
            focus,
            focus_state,
            frontier,
            workstreams,
            persistent: engine.persistence_enabled(),
        })
    }

    pub fn focus_set(
        &self,
        lane: &str,
        group: &str,
        mode: FocusMode,
    ) -> Result<Observed<FocusChange>, RoadmapError> {
        let mut engine = self.lock()?;
        let focus = engine
            .focus_set(lane, group, mode)
            .map_err(RoadmapError::invalid_args)?;
        let frontier = engine.workstream_frontier(Some(&focus.value.group));
        Ok(focus.map(|focus| FocusChange::Set {
            lane: focus.lane.clone(),
            focus,
            frontier,
        }))
    }

    pub fn focus_clear(&self, lane: &str) -> Result<Observed<FocusChange>, RoadmapError> {
        validate_lane(lane).map_err(RoadmapError::InvalidArgs)?;
        let mut engine = self.lock()?;
        let released = engine
            .focus_clear(lane)
            .map_err(RoadmapError::invalid_args)?;
        Ok(released.map(|released| FocusChange::Cleared {
            lane: lane.to_owned(),
            focus: None,
            released,
        }))
    }

    pub fn propose_groups(&self, min_shared: usize) -> Result<GroupProposal, RoadmapError> {
        let engine = self.lock()?;
        Ok(GroupProposal {
            clusters: engine.propose_groups_from_ids(min_shared),
            min_shared,
            authored_names: engine.workstreams().authored_names(),
            placed: 0,
        })
    }
}

fn inspect_exact(
    engine: &RoadmapEngine,
    workstreams: &WorkstreamCatalog,
    focus_state: &FocusState,
    view: WorkstreamView,
) -> WorkstreamInspection {
    match focus_state {
        FocusState::Unset => WorkstreamInspection::Unset,
        FocusState::Current { group } | FocusState::Closed { group }
            if view == WorkstreamView::History =>
        {
            WorkstreamInspection::History {
                group: group.clone(),
                history: engine.workstream_history(Some(group)),
            }
        }
        FocusState::Current { group } => WorkstreamInspection::Current {
            frontier: engine.workstream_frontier(Some(group)),
        },
        FocusState::Closed { group } => WorkstreamInspection::Closed {
            group: group.clone(),
            history: workstreams
                .history
                .get(group)
                .expect("closed workstream was classified from this catalog")
                .clone(),
        },
        FocusState::Missing { group } => WorkstreamInspection::Missing {
            query: group.clone(),
        },
    }
}
