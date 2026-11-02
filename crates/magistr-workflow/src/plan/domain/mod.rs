//! Canonical effect-free records and projections used by the plan owner.
pub use magistr_workflow_contracts::plan::{
    ContainerKind, ContainerLink, FeatureActivity, FeatureCatalog, FeatureCounts,
    FeatureDependency, FeatureFrontier, FeatureHistory, FeatureHistoryRecords, FeatureItem,
    FeatureReadiness, FeatureReadinessCounts, FeatureResolution, Focus, FocusMode, FocusState,
    NoteSection, Plan, Readiness, RefreshNote, ReprioritizeProposal, StatusProposal, TEAM_BUDGET,
    Task, TaskStatus, TitleProposal, TrackerLink, TrackerOptIn, WaitingDependency, WaitingOn,
    WaitingReason, assess_readiness, feature_catalog, feature_frontier, feature_history,
    merge_plans, next_ready, rfc3339_newer, task_wins, validate_team,
};
