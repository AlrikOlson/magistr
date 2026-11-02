use crate::plan::domain::{
    FeatureCatalog, FeatureResolution, FocusMode, FocusState, validate_team,
};
use crate::plan::engine::PlanEngine;
use magistr_workflow_contracts::Observed;

use super::{
    FeatureInspection, FeatureProposal, FeatureView, FocusChange, FocusRead, PlanError,
    PlanUseCases,
};

impl PlanUseCases {
    pub fn focus_get(
        &self,
        team: &str,
        feature: Option<&str>,
        view: FeatureView,
    ) -> Result<FocusRead, PlanError> {
        let engine = self.lock()?;
        let features = engine.features();
        let focus = engine.focus_get(team).cloned();
        let focus_state = features.focus_state(focus.as_ref().map(|focus| focus.feature.as_str()));
        let frontier = match feature.map(str::trim).filter(|feature| !feature.is_empty()) {
            Some(query) => match features.resolve_current(query) {
                FeatureResolution::Current { feature }
                | FeatureResolution::Closed { feature, .. } => inspect_exact(
                    &engine,
                    &features,
                    &features.focus_state(Some(&feature)),
                    view,
                ),
                FeatureResolution::Unknown { .. } => FeatureInspection::Missing {
                    query: query.to_owned(),
                },
                FeatureResolution::Ambiguous { matches } => FeatureInspection::Ambiguous {
                    query: query.to_owned(),
                    matches,
                },
            },
            None => inspect_exact(&engine, &features, &focus_state, view),
        };
        Ok(FocusRead {
            team: team.to_owned(),
            focus,
            focus_state,
            frontier,
            features,
            persistent: engine.persistence_enabled(),
        })
    }

    pub fn focus_set(
        &self,
        team: &str,
        feature: &str,
        mode: FocusMode,
    ) -> Result<Observed<FocusChange>, PlanError> {
        let mut engine = self.lock()?;
        let focus = engine
            .focus_set(team, feature, mode)
            .map_err(PlanError::invalid_args)?;
        let frontier = engine.feature_frontier(Some(&focus.value.feature));
        Ok(focus.map(|focus| FocusChange::Set {
            team: focus.team.clone(),
            focus,
            frontier,
        }))
    }

    pub fn focus_clear(&self, team: &str) -> Result<Observed<FocusChange>, PlanError> {
        validate_team(team).map_err(PlanError::InvalidArgs)?;
        let mut engine = self.lock()?;
        let released = engine.focus_clear(team).map_err(PlanError::invalid_args)?;
        Ok(released.map(|released| FocusChange::Cleared {
            team: team.to_owned(),
            focus: None,
            released,
        }))
    }

    pub fn propose_features(&self, min_shared: usize) -> Result<FeatureProposal, PlanError> {
        let engine = self.lock()?;
        Ok(FeatureProposal {
            clusters: engine.propose_features_from_ids(min_shared),
            min_shared,
            authored_names: engine.features().authored_names(),
            placed: 0,
        })
    }
}

fn inspect_exact(
    engine: &PlanEngine,
    features: &FeatureCatalog,
    focus_state: &FocusState,
    view: FeatureView,
) -> FeatureInspection {
    match focus_state {
        FocusState::Unset => FeatureInspection::Unset,
        FocusState::Current { feature } | FocusState::Closed { feature }
            if view == FeatureView::History =>
        {
            FeatureInspection::History {
                feature: feature.clone(),
                history: engine.feature_history(Some(feature)),
            }
        }
        FocusState::Current { feature } => FeatureInspection::Current {
            frontier: engine.feature_frontier(Some(feature)),
        },
        FocusState::Closed { feature } => FeatureInspection::Closed {
            feature: feature.clone(),
            history: features
                .history
                .get(feature)
                .expect("closed feature was classified from this catalog")
                .clone(),
        },
        FocusState::Missing { feature } => FeatureInspection::Missing {
            query: feature.clone(),
        },
    }
}
