//! Canonical workflow records and projection values, independent of stores and runtimes.
//! Producers and surfaces share these definitions; host crates only re-export them.

#![forbid(unsafe_code)]

mod content;
pub use content::{
    CONTENT_VERSION, ContentFact, ContentListItem, ContentSection, StructuredContent, input_schema,
    parse_optional,
};
mod family;
mod mutation_error;
pub use mutation_error::WorkflowMutationError;
pub mod decision;
pub mod plan;
pub mod request;
pub mod run;
pub use family::Family;
mod broadcast_frame;
mod check_bytes;
mod decision_thread;
mod delivery;
mod event_cursor;
mod feed_cursor;
mod feed_status;
mod magistr_frame;
mod native_event;
mod native_snapshot;
mod owner_generation;
mod plan_progress;
mod read_state;
mod store_stamp;
mod write_error;
pub use write_error::{WriteError, WriteErrorKind};

pub use broadcast_frame::NativeFrame;
pub use check_bytes::CheckBytes;
pub use decision_thread::DecisionThread;
pub use delivery::Delivery;
pub use event_cursor::EventCursor;
pub use feed_cursor::FeedCursor;
pub use feed_status::FeedStatus;
pub use magistr_frame::MagistrFrame;
pub use native_event::NativeEvent;
pub use native_snapshot::{DecisionData, NativeSnapshot, PlanData, Run, RunData};
pub use owner_generation::OwnerGeneration;
pub use plan_progress::PlanProgress;
pub use read_state::{Observed, ReadError, ReadErrorKind, ReadSource, ReadState};
pub use store_stamp::StoreStamp;

pub use decision::{DecisionFrame, DepEdge, NextAction, Note, ThreadStatus};
pub use plan::{
    PLAN_SCHEMA_VERSION, PlanFrame, ReprioritizeProposal, StatusProposal, Task, TaskStatus,
    TitleProposal, WaitingOn, WaitingReason,
};
pub use request::{
    Enrichment, REQUESTS_SCHEMA_VERSION, Request, RequestFrame, RequestKind, RequestStatus,
};
pub use run::{
    Artifact, ArtifactType, Check, CheckType, Goal, GoalStatus, Progress, ProgressType, RunFrame,
    Step, StepStatus, StepType, VerificationMode,
};

pub use family::{EPOCH_KEY, SEQ_KEY};
