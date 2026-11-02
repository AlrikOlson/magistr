//! Pure domain types — no logic, no IO, no MCP wire concerns.
//!
//! These types are the contract between the engine, persistence, broadcast,
//! and the Tauri viewer. They're deliberately small and trait-free so the
//! viewer can deserialize the same bytes that the engine produced.

pub mod note_error;
pub mod recorded_note;
pub use note_error::NoteError;
pub use recorded_note::RecordedNote;

pub mod dep_edge;
pub mod log;
pub mod log_entry;
pub mod note;
pub mod thread;

pub use dep_edge::DepEdge;
pub use log::{Log, LogMetadata};
pub use log_entry::LogEntry;
pub use note::{NextAction, Note, StructuredAction};
pub use thread::{Thread, ThreadStatus, ThreadStatusChange};
