//! Canonical plan records and typed mutation events, without host effects.

mod features;
mod frame;
mod name;
mod readiness;
mod records;
pub use name::{NAME_BUDGET, derive, fits, repair_collisions, why_unfit};
pub use readiness::{Readiness, WaitingDependency, assess_readiness, next_ready};

pub use features::{
    FeatureActivity, FeatureCatalog, FeatureCounts, FeatureDependency, FeatureFrontier,
    FeatureHistory, FeatureHistoryRecords, FeatureItem, FeatureReadiness, FeatureReadinessCounts,
    FeatureResolution, FocusState, feature_catalog, feature_frontier, feature_history,
};

pub use frame::PlanFrame;
pub use records::{
    ContainerKind, ContainerLink, Focus, FocusMode, NoteSection, PLAN_SCHEMA_VERSION, Plan,
    RefreshNote, ReprioritizeProposal, StatusProposal, TEAM_BUDGET, Task, TaskStatus,
    TitleProposal, TrackerLink, TrackerOptIn, WaitingOn, WaitingReason, merge_plans, rfc3339_newer,
    task_wins, validate_team,
};
