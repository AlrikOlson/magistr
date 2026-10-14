//! Canonical roadmap records and typed mutation events, without host effects.

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

pub use frame::RoadmapFrame;
pub use records::{
    BlockedBy, BlockerKind, Chunk, ChunkStatus, ContainerKind, ContainerLink, Focus, FocusMode,
    LANE_BUDGET, NoteSection, RefreshNote, ReprioritizeProposal, Roadmap, StatusProposal,
    TitleProposal, TrackerLink, TrackerOptIn, chunk_wins, merge_roadmaps, rfc3339_newer,
    validate_lane,
};
