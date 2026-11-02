use crate::plan::domain::{
    FocusMode, FocusState, WorkstreamCatalog, WorkstreamResolution, validate_lane,
};
use crate::plan::engine::PlanEngine;
use magistr_workflow_contracts::Observed;

use super::{
    FeatureProposal, FocusChange, FocusRead, PlanError, PlanUseCases, WorkstreamInspection,
    WorkstreamView,
};

impl PlanUseCases {
    pub fn focus_get(
        &self,
        lane: &str,
        feature: Option<&str>,
        view: WorkstreamView,
    ) -> Result<FocusRead, PlanError> {
        let engine = self.lock()?;
        let workstreams = engine.workstreams();
        let focus = engine.focus_get(lane).cloned();
        let focus_state =
            workstreams.focus_state(focus.as_ref().map(|focus| focus.feature.as_str()));
        let frontier = match feature.map(str::trim).filter(|feature| !feature.is_empty()) {
            Some(query) => match workstreams.resolve_current(query) {
                WorkstreamResolution::Current { feature }
                | WorkstreamResolution::Closed { feature, .. } => inspect_exact(
                    &engine,
                    &workstreams,
                    &workstreams.focus_state(Some(&feature)),
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
        feature: &str,
        mode: FocusMode,
    ) -> Result<Observed<FocusChange>, PlanError> {
        let mut engine = self.lock()?;
        let focus = engine
            .focus_set(lane, feature, mode)
            .map_err(PlanError::invalid_args)?;
        let frontier = engine.workstream_frontier(Some(&focus.value.feature));
        Ok(focus.map(|focus| FocusChange::Set {
            lane: focus.lane.clone(),
            focus,
            frontier,
        }))
    }

    pub fn focus_clear(&self, lane: &str) -> Result<Observed<FocusChange>, PlanError> {
        validate_lane(lane).map_err(PlanError::InvalidArgs)?;
        let mut engine = self.lock()?;
        let released = engine.focus_clear(lane).map_err(PlanError::invalid_args)?;
        Ok(released.map(|released| FocusChange::Cleared {
            lane: lane.to_owned(),
            focus: None,
            released,
        }))
    }

    pub fn propose_features(&self, min_shared: usize) -> Result<FeatureProposal, PlanError> {
        let engine = self.lock()?;
        Ok(FeatureProposal {
            clusters: engine.propose_features_from_ids(min_shared),
            min_shared,
            authored_names: engine.workstreams().authored_names(),
            placed: 0,
        })
    }
}

fn inspect_exact(
    engine: &PlanEngine,
    workstreams: &WorkstreamCatalog,
    focus_state: &FocusState,
    view: WorkstreamView,
) -> WorkstreamInspection {
    match focus_state {
        FocusState::Unset => WorkstreamInspection::Unset,
        FocusState::Current { feature } | FocusState::Closed { feature }
            if view == WorkstreamView::History =>
        {
            WorkstreamInspection::History {
                feature: feature.clone(),
                history: engine.workstream_history(Some(feature)),
            }
        }
        FocusState::Current { feature } => WorkstreamInspection::Current {
            frontier: engine.workstream_frontier(Some(feature)),
        },
        FocusState::Closed { feature } => WorkstreamInspection::Closed {
            feature: feature.clone(),
            history: workstreams
                .history
                .get(feature)
                .expect("closed workstream was classified from this catalog")
                .clone(),
        },
        FocusState::Missing { feature } => WorkstreamInspection::Missing {
            query: feature.clone(),
        },
    }
}
