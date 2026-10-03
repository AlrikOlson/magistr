//! The native workflow store: Magistr's think-and-ship fork, composed in process.
//!
//! Magistr links the `magistr_workflow` library (the repository-local fork of
//! think-and-ship, see `subsystems/workflow/PROVENANCE.md`) and runs its four engines —
//! roadmap, think (reasoning trace), ship (execution trace) and signal — in
//! this process. State lives in the same XDG store, keyed by the same
//! `.think-and-ship/project.json` identity retained from the owned fork.
//! Agent tools and native views use Magistr's admitted owners; no standalone
//! think-and-ship service is launched. The fork preserves its locked merge-on-save
//! discipline (advisory lock, merge, atomic rename); nothing here re-implements
//! persistence or identity.
//!
//! Writers are admitted by a [`ProjectAuthority`] — one per [`WriterRole`]
//! per project and store, cooperatively — and receive the write capability
//! [`NativeEngines`]; anyone may open the read capability [`NativeReader`].
//!
//! Canonical values live in `magistr-workflow-contracts`; this adapter produces
//! facts from the native store using those exact types. A
//! [`NativeSnapshot`] is the store read whole off a reader, a [`NativeEvent`]
//! is one change as the engines say it (the workflow's own frame enums plus
//! the host's [`MagistrFrame`]), and the socket line every publisher writes
//! is read back into one of those only by [`NativeFrame::decode`], with the
//! publisher's [`EventCursor`] beside it for a [`FeedCursor`] to classify.
//!
//! The module is named for its responsibility — the native workflow store and
//! its owner protocol — beside the engine's own `workflow` phase engine.

pub mod attach_error;
pub mod attach_kind;
pub mod attach_refusal;
pub mod broadcast_frame;
pub mod build_identity;
pub mod caller_identity;
mod check_type_for;
pub mod delivery;
pub mod event_cursor;
mod family_services;
pub mod feed_cursor;
pub mod feed_status;
pub mod gate_output;
pub mod identity;
mod local_stream;
pub mod magistr_frame;
pub mod mcp_acceptor;
pub mod native_config;
pub mod native_engines;
pub mod native_event;
pub mod native_reader;
pub mod native_snapshot;
mod open_engines;
pub mod owner_client;
pub mod owner_command;
pub mod owner_connection_record;
#[cfg(any(unix, windows))]
mod owner_connections;
mod owner_dispatch;
pub mod owner_endpoint;
pub mod owner_generation;
pub mod owner_identity;
mod owner_lease;
pub mod owner_liveness;
#[cfg(windows)]
mod owner_pipe;
pub mod owner_probe;
pub mod owner_reply;
pub mod owner_report;
pub mod owner_socket;
#[cfg(unix)]
mod owner_socket_file;
pub mod owner_wire;
#[cfg(any(unix, windows))]
mod owner_workspace_link;
pub mod progress;
pub mod project_authority;
pub mod region_counts;
pub mod roadmap_backend;
pub mod roadmap_command;
pub mod ship_command;
pub mod signal_command;
pub mod snapshot;
pub mod snapshot_cursor;
pub mod think_branch;
pub mod think_command;
#[cfg(windows)]
mod user_sid;
pub mod workspace_socket_path;
pub mod writer_role;

pub use attach_error::AttachError;
pub use attach_kind::AttachKind;
pub use attach_refusal::AttachRefusal;
pub use broadcast_frame::NativeFrame;
pub use build_identity::BuildIdentity;
pub use caller_identity::CallerIdentity;
pub use check_type_for::check_type_for;
pub use delivery::Delivery;
pub use event_cursor::EventCursor;
pub use feed_cursor::FeedCursor;
pub use feed_status::FeedStatus;
pub use gate_output::GateOutput;
pub use identity::identity_declaration;
pub use local_stream::LocalStream;
pub use magistr_frame::MagistrFrame;
pub use magistr_workflow::think::ConfigSource;
pub use mcp_acceptor::McpAcceptor;
pub use native_config::NativeConfig;
pub use native_engines::NativeEngines;
pub use native_event::NativeEvent;
pub use native_reader::NativeReader;
pub use native_snapshot::NativeSnapshot;
pub use owner_client::OwnerClient;
pub use owner_command::OwnerCommand;
pub use owner_connection_record::OwnerConnectionRecord;
pub use owner_endpoint::OwnerEndpoint;
pub use owner_generation::OwnerGeneration;
pub use owner_identity::OwnerIdentity;
pub use owner_liveness::OwnerLiveness;
pub use owner_probe::OwnerProbe;
pub use owner_reply::OwnerReply;
pub use owner_report::OwnerReport;
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
pub use snapshot_cursor::SnapshotCursor;
pub use think_branch::ThinkBranch;
pub use think_command::ThinkCommand;
pub use workspace_socket_path::workspace_socket_path;
pub use writer_role::WriterRole;

// The library types the typed commands speak, re-exported so consumers
// (magistr-core, the GUI, the TUI) never name the `magistr_workflow` crate.
pub use magistr_workflow::content::{
    ContentFact, ContentListItem, ContentSection, StructuredContent,
};
pub use magistr_workflow::infra::{
    Broadcaster, Family, IdSource, PROJECT_DIR, PROJECT_FILE, ProjectIdentity, project_id_for_path,
    resolve_project_id_with,
};
// The workflow's own typed frames — what its engines hand their broadcaster
// — are the events every surface folds, under the family's name.
pub use magistr_workflow::roadmap::broadcast::RoadmapFrame;
pub use magistr_workflow::roadmap::domain::{
    BlockedBy, BlockerKind, Chunk, ChunkStatus, ReprioritizeProposal, Roadmap, StatusProposal,
    TitleProposal,
};
pub use magistr_workflow::ship::application::{
    CheckAdmission, CheckExecution, CheckSettlement, CompletionEvidence, CompletionRefusal,
    EvidencePort, GateContract, ObservationDrift, ShipReport, SourceFreshness, SourceObservation,
    SourceWitness, TaskAdmission,
};
pub use magistr_workflow::ship::broadcast::BroadcastFrame as ShipFrame;
pub use magistr_workflow::ship::domain::action::{Action, ActionType};
pub use magistr_workflow::ship::domain::artifact::{Artifact, ArtifactType};
pub use magistr_workflow::ship::domain::check::{Check, CheckType};
pub use magistr_workflow::ship::domain::check_receipt::CheckReceipt;
pub use magistr_workflow::ship::domain::objective::{Objective, ObjectiveStatus, VerificationMode};
pub use magistr_workflow::ship::domain::receipt_settlement::ReceiptSettlement;
pub use magistr_workflow::ship::domain::task::{Task, TaskStatus, TaskType};
pub use magistr_workflow::ship::receipt_compaction::ReceiptCompaction;
pub use magistr_workflow::ship::report::{ReportRecord, TestResults};
pub use magistr_workflow::signal::broadcast::SignalFrame;
pub use magistr_workflow::signal::domain::{Enrichment, Signal, SignalKind, SignalStatus};
pub use magistr_workflow::think::broadcast::BroadcastFrame as ThinkFrame;
pub use magistr_workflow::think::domain::{BranchStatus, DepEdge, NextAction, ThinkStep};
pub use magistr_workflow::think::domain::{RecordedStep, StepError};
