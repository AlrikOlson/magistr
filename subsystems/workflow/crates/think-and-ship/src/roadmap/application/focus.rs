use crate::roadmap::domain::{FocusMode, validate_lane};
use crate::roadmap::engine::GroupResolution;

use super::{FocusChange, FocusRead, GroupProposal, RoadmapError, RoadmapUseCases};

impl RoadmapUseCases {
    pub fn focus_get(&self, lane: &str, group: Option<&str>) -> Result<FocusRead, RoadmapError> {
        let engine = self.lock()?;
        let focus = engine.focus_get(lane).cloned();
        let subject = group
            .map(str::trim)
            .filter(|group| !group.is_empty())
            .map(str::to_owned)
            .or_else(|| focus.as_ref().map(|focus| focus.group.clone()));
        let frontier = match subject {
            Some(group) => match engine.resolve_group(&group) {
                GroupResolution::Exact(name) => engine.group_status(&name),
                other => serde_json::json!({"error": other.explain(&group)}),
            },
            None => serde_json::Value::Null,
        };
        Ok(FocusRead {
            lane: lane.to_owned(),
            focus,
            frontier,
            groups: engine.groups(),
            persistent: engine.persistence_enabled(),
        })
    }

    pub fn focus_set(
        &self,
        lane: &str,
        group: &str,
        mode: FocusMode,
    ) -> Result<FocusChange, RoadmapError> {
        let mut engine = self.lock()?;
        let focus = engine
            .focus_set(lane, group, mode)
            .map_err(RoadmapError::InvalidArgs)?;
        let frontier = engine.group_status(&focus.group);
        Ok(FocusChange::Set {
            lane: focus.lane.clone(),
            focus,
            frontier,
            persistent: engine.persistence_enabled(),
        })
    }

    pub fn focus_clear(&self, lane: &str) -> Result<FocusChange, RoadmapError> {
        validate_lane(lane).map_err(RoadmapError::InvalidArgs)?;
        let mut engine = self.lock()?;
        let released = engine.focus_clear(lane);
        Ok(FocusChange::Cleared {
            lane: lane.to_owned(),
            focus: None,
            released,
            persistent: engine.persistence_enabled(),
        })
    }

    pub fn propose_groups(&self, min_shared: usize) -> Result<GroupProposal, RoadmapError> {
        let engine = self.lock()?;
        Ok(GroupProposal {
            clusters: engine.propose_groups_from_ids(min_shared),
            min_shared,
            groups: engine.groups(),
            placed: 0,
        })
    }
}
