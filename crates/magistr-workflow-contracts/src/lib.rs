//! Canonical workflow records and projection values, independent of stores and runtimes.
//! Producers and surfaces share these definitions; host crates only re-export them.

#![forbid(unsafe_code)]

mod content;
pub use content::{
    CONTENT_VERSION, ContentFact, ContentListItem, ContentSection, StructuredContent, input_schema,
    parse_optional,
};
mod family;
pub mod roadmap;
pub mod ship;
pub mod signal;
pub mod think;
pub use family::Family;
mod broadcast_frame;
mod delivery;
mod event_cursor;
mod feed_cursor;
mod feed_status;
mod gate_output;
mod magistr_frame;
mod native_event;
mod native_snapshot;
mod owner_generation;
mod roadmap_progress;
mod snapshot_cursor;
mod think_branch;

pub use broadcast_frame::NativeFrame;
pub use delivery::Delivery;
pub use event_cursor::EventCursor;
pub use feed_cursor::FeedCursor;
pub use feed_status::FeedStatus;
pub use gate_output::GateOutput;
pub use magistr_frame::MagistrFrame;
pub use native_event::NativeEvent;
pub use native_snapshot::NativeSnapshot;
pub use owner_generation::OwnerGeneration;
pub use roadmap_progress::RoadmapProgress;
pub use snapshot_cursor::SnapshotCursor;
pub use think_branch::ThinkBranch;

pub use roadmap::{
    BlockedBy, BlockerKind, Chunk, ChunkStatus, ReprioritizeProposal, RoadmapFrame, StatusProposal,
    TitleProposal,
};
pub use ship::{
    Action, ActionType, Artifact, ArtifactType, Check, CheckType, Objective, ObjectiveStatus,
    ShipFrame, Task, TaskStatus, TaskType, VerificationMode,
};
pub use signal::{Enrichment, Signal, SignalFrame, SignalKind, SignalStatus};
pub use think::{BranchStatus, DepEdge, NextAction, ThinkFrame, ThinkStep};

pub use family::{EPOCH_KEY, SEQ_KEY};
