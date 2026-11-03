//! Magistr's native workflow store and owner protocol.
//!
//! Magistr links its `magistr_workflow` library and runs its four engines —
//! plan, decision (decision log), run (run history) and request — in
//! this process. State lives under the resolved Magistr workflow data root,
//! keyed by `project.id` in `.magistr/project.json`.
//! Agent tools and native views use Magistr's admitted owners; no standalone
//! workflow service is launched. Persistence uses locked merge-on-save
//! discipline (advisory lock, merge, atomic rename); nothing here re-implements
//! persistence or identity.
//!
//! Writers are admitted by a [`ProjectAuthority`] — one per project across
//! host kinds and linked working copies, cooperatively — and receive the write capability
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
mod execution_context;
mod family_services;
mod store_words_upgrade;
mod workflow_upgrade;
pub use store_words_upgrade::StoreWordsUpgrade;
pub use workflow_upgrade::WorkflowUpgrade;
pub mod check_bytes;
pub mod feed_cursor;
pub mod feed_status;
mod local_stream;
pub mod magistr_frame;
pub mod mcp_acceptor;
pub use magistr_workflow_contracts::{WorkflowMutationError, request::RequestPromotion};
pub mod native_config;
pub mod native_engines;
pub mod native_event;
pub mod native_reader;
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
mod project_location;
mod working_copy_lease;
pub use working_copy_lease::WorkingCopyLease;
pub mod decision_command;
pub mod decision_thread;
pub mod plan_backend;
pub mod plan_command;
pub mod request_command;
pub mod run_command;
pub mod snapshot;
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
pub use check_bytes::CheckBytes;
pub use check_type_for::check_type_for;
pub use decision_command::DecisionCommand;
pub use decision_thread::DecisionThread;
pub use delivery::Delivery;
pub use event_cursor::EventCursor;
pub use execution_context::ExecutionContext;
pub use feed_cursor::FeedCursor;
pub use feed_status::FeedStatus;
pub use local_stream::LocalStream;
pub use magistr_frame::MagistrFrame;
pub use magistr_workflow::decision::ConfigSource;
pub use magistr_workflow_contracts::{
    DecisionData, NativeSnapshot, Observed, PlanData, ReadError, ReadErrorKind, ReadSource,
    ReadState, Run, RunData, StoreStamp,
};
pub use mcp_acceptor::McpAcceptor;
pub use native_config::NativeConfig;
pub use native_engines::NativeEngines;
pub use native_event::NativeEvent;
pub use native_reader::NativeReader;
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
pub use plan_backend::PlanBackend;
pub use plan_command::PlanCommand;
pub use progress::progress_from_tasks;
pub use project_authority::ProjectAuthority;
pub use project_location::ProjectLocation;
pub use request_command::RequestCommand;
pub use run_command::RunCommand;
pub use snapshot::NativePlanSnapshot;
pub use workspace_socket_path::workspace_socket_path;
pub use writer_role::WriterRole;

// The library types the typed commands speak, re-exported so consumers
// (magistr-core, the GUI, the interface) never name the `magistr_workflow` crate.
pub use magistr_workflow::content::{
    ContentFact, ContentListItem, ContentSection, StructuredContent,
};
pub use magistr_workflow::infra::{
    Broadcaster, Family, IdSource, PROJECT_DIR, PROJECT_FILE, ProjectIdentity, project_id_for_path,
    resolve_project_id_with,
};
// The workflow's own typed frames — what its engines hand their broadcaster
// — are the events every surface folds, under the family's name.
pub use magistr_workflow::decision::broadcast::BroadcastFrame as DecisionFrame;
pub use magistr_workflow::decision::domain::{DepEdge, NextAction, Note, ThreadStatus};
pub use magistr_workflow::decision::domain::{NoteError, RecordedNote};
pub use magistr_workflow::plan::broadcast::PlanFrame;
pub use magistr_workflow::plan::domain::{
    FeatureActivity, FeatureCatalog, FeatureFrontier, FeatureHistory, FeatureResolution,
    FocusState, Plan, Readiness, ReprioritizeProposal, StatusProposal, Task, TaskStatus,
    TitleProposal, WaitingOn, WaitingReason, assess_readiness, feature_catalog, feature_frontier,
    next_ready,
};
pub use magistr_workflow::request::broadcast::RequestFrame;
pub use magistr_workflow::request::domain::{Enrichment, Request, RequestKind, RequestStatus};
pub use magistr_workflow::run::application::{
    CheckAdmission, CheckBudget, CheckContract, CheckExecution, CheckOptions, CheckOutcome,
    CheckRequest, CheckSettlement, CompletionEvidence, CompletionRefusal, EvidencePort,
    ExecutionBinding, ObservationDrift, RunReport, SourceFreshness, SourceObservation,
    SourceWitness, StepAdmission, TimeoutOrigin,
};
pub use magistr_workflow::run::broadcast::BroadcastFrame as RunFrame;
pub use magistr_workflow::run::check_result_compaction::CheckResultCompaction;
pub use magistr_workflow::run::domain::artifact::{Artifact, ArtifactType};
pub use magistr_workflow::run::domain::check::{Check, CheckType};
pub use magistr_workflow::run::domain::check_result::CheckResult;
pub use magistr_workflow::run::domain::check_result_settlement::CheckResultSettlement;
pub use magistr_workflow::run::domain::goal::{Goal, GoalStatus, VerificationMode};
pub use magistr_workflow::run::domain::progress::{Progress, ProgressType};
pub use magistr_workflow::run::domain::step::{Step, StepStatus, StepType};
pub use magistr_workflow::run::report::{ReportRecord, TestResults};
