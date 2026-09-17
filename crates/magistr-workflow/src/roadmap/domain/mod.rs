//! Compatibility exports of the canonical effect-free workflow contracts.
pub use magistr_workflow_contracts::roadmap::{
    BlockedBy, BlockerKind, Chunk, ChunkStatus, ContainerKind, ContainerLink, Focus, FocusMode,
    LANE_BUDGET, NoteSection, RefreshNote, ReprioritizeProposal, Roadmap, StatusProposal,
    TitleProposal, TrackerLink, TrackerOptIn, chunk_wins, merge_roadmaps, rfc3339_newer,
    validate_lane,
};
