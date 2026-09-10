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
//! Writers are admitted by a [`ProjectAuthority`] — one per [`WriterRole`]
//! per project and store, cooperatively — and receive the write capability
//! [`NativeEngines`]; anyone may open the read capability [`NativeReader`].
//!
//! The module name is short for "think-and-ship"; `think_and_ship` itself is
//! the extern crate and would shadow it.

pub mod attach_error;
pub mod attach_kind;
pub mod attach_refusal;
pub mod broadcast_frame;
pub mod caller_identity;
mod family_services;
pub mod feed_status;
pub mod gate_output;
pub mod identity;
pub mod mcp_acceptor;
pub mod native_engines;
pub mod native_reader;
mod open_engines;
pub mod owner_client;
pub mod owner_command;
mod owner_dispatch;
pub mod owner_endpoint;
pub mod owner_generation;
mod owner_lease;
pub mod owner_reply;
pub mod owner_socket;
pub mod owner_wire;
pub mod progress;
pub mod project_authority;
pub mod region_counts;
pub mod roadmap_backend;
pub mod roadmap_command;
pub mod ship_command;
pub mod signal_command;
pub mod snapshot;
pub mod subscriber;
pub mod tas_config;
pub mod think_command;
pub mod writer_role;

pub use attach_error::AttachError;
pub use attach_kind::AttachKind;
pub use attach_refusal::AttachRefusal;
pub use broadcast_frame::NativeFrame;
pub use caller_identity::CallerIdentity;
pub use feed_status::FeedStatus;
pub use gate_output::GateOutput;
pub use identity::ensure_project_file;
pub use mcp_acceptor::McpAcceptor;
pub use native_engines::NativeEngines;
pub use native_reader::NativeReader;
pub use owner_client::OwnerClient;
pub use owner_command::OwnerCommand;
pub use owner_endpoint::OwnerEndpoint;
pub use owner_generation::OwnerGeneration;
pub use owner_reply::OwnerReply;
pub use owner_socket::owner_socket_path;
pub use owner_wire::OWNER_PROTOCOL_VERSION;
pub use progress::progress_from_roadmap;
pub use project_authority::ProjectAuthority;
pub use region_counts::RegionCounts;
pub use roadmap_backend::RoadmapBackend;
pub use roadmap_command::RoadmapCommand;
pub use ship_command::ShipCommand;
pub use signal_command::SignalCommand;
pub use snapshot::NativeRoadmapSnapshot;
pub use subscriber::spawn_broadcast_subscriber;
pub use tas_config::TasConfig;
pub use think_command::ThinkCommand;
pub use writer_role::WriterRole;

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
pub use think_and_ship::ship::application::{
    CheckAdmission, CheckSettlement, ShipReport, TaskAdmission,
};
pub use think_and_ship::ship::domain::action::{Action, ActionType};
pub use think_and_ship::ship::domain::artifact::{Artifact, ArtifactType};
pub use think_and_ship::ship::domain::check::{Check, CheckType};
pub use think_and_ship::ship::domain::objective::Objective;
pub use think_and_ship::ship::domain::task::{Task, TaskType};
pub use think_and_ship::signal::domain::{Signal, SignalKind};
pub use think_and_ship::think::domain::ThinkStep;
pub use think_and_ship::think::domain::{RecordedStep, StepError};
