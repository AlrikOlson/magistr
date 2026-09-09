//! Native think-and-ship subsystem.
//!
//! Magistr links the `think_and_ship` library and runs its four engines —
//! roadmap, think (reasoning trace), ship (execution trace) and signal — in
//! this process. State lives in the same XDG store, keyed by the same
//! `.think-and-ship/project.json` identity, that an external
//! `think-and-ship` MCP server uses, so agent writes over MCP and Magistr's
//! native views see one roadmap. Concurrent writers are safe by the
//! library's locked merge-on-save discipline (advisory lock, merge, atomic
//! rename); nothing here re-implements persistence or identity.
//!
//! The module name is short for "think-and-ship"; `think_and_ship` itself is
//! the extern crate and would shadow it.

pub mod broadcast_frame;
pub mod feed_status;
pub mod gate_output;
pub mod identity;
pub mod native_engines;
pub mod open_engines;
pub mod progress;
pub mod region_counts;
pub mod roadmap_backend;
pub mod snapshot;
pub mod subscriber;
pub mod tas_config;

pub use broadcast_frame::NativeFrame;
pub use feed_status::FeedStatus;
pub use gate_output::GateOutput;
pub use identity::ensure_project_file;
pub use native_engines::NativeEngines;
pub use open_engines::open_engines;
pub use progress::progress_from_roadmap;
pub use region_counts::RegionCounts;
pub use roadmap_backend::RoadmapBackend;
pub use snapshot::NativeRoadmapSnapshot;
pub use subscriber::spawn_broadcast_subscriber;
pub use tas_config::TasConfig;

// The library types the typed commands speak, re-exported so consumers
// (magistr-core, the GUI, the TUI) never name the `think_and_ship` crate.
pub use think_and_ship::content::{
    ContentFact, ContentListItem, ContentSection, StructuredContent,
};
pub use think_and_ship::infra::{
    Broadcaster, IdSource, PROJECT_DIR, PROJECT_FILE, ProjectIdentity, resolve_project_id_with,
};
pub use think_and_ship::roadmap::domain::{
    BlockedBy, BlockerKind, Chunk, ChunkStatus, ReprioritizeProposal, Roadmap, StatusProposal,
    TitleProposal,
};
pub use think_and_ship::ship::domain::action::{Action, ActionType};
pub use think_and_ship::ship::domain::artifact::{Artifact, ArtifactType};
pub use think_and_ship::ship::domain::check::{Check, CheckType};
pub use think_and_ship::ship::domain::objective::Objective;
pub use think_and_ship::ship::domain::task::{Task, TaskType};
pub use think_and_ship::signal::domain::{Signal, SignalKind};
pub use think_and_ship::think::domain::ThinkStep;
pub use think_and_ship::think::engine::{ProcessErr, ProcessOk, ProcessResult};
