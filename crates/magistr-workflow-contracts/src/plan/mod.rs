//! Canonical plan records and typed mutation events, without host effects.

mod frame;
mod name;
mod readiness;
mod records;
mod workstreams;
pub use name::{NAME_BUDGET, derive, fits, repair_collisions, why_unfit};
pub use readiness::{Readiness, WaitingDependency, assess_readiness, next_ready};

pub use workstreams::{
    FocusState, WorkstreamActivity, WorkstreamCatalog, WorkstreamCounts, WorkstreamDependency,
    WorkstreamFrontier, WorkstreamHistory, WorkstreamHistoryRecords, WorkstreamItem,
    WorkstreamReadiness, WorkstreamReadinessCounts, WorkstreamResolution, workstream_catalog,
    workstream_frontier, workstream_history,
};

pub use frame::PlanFrame;
pub use records::{
    ContainerKind, ContainerLink, Focus, FocusMode, LANE_BUDGET, NoteSection, PLAN_SCHEMA_VERSION,
    Plan, RefreshNote, ReprioritizeProposal, StatusProposal, Task, TaskStatus, TitleProposal,
    TrackerLink, TrackerOptIn, WaitingOn, WaitingReason, merge_plans, rfc3339_newer, task_wins,
    validate_lane,
};
