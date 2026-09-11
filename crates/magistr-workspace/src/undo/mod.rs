//! Pure exact-byte undo planning. Plans do not apply writes or consume history.

mod conflict;
mod conflict_policy;
mod plan;
mod plan_error;
mod skipped_file;
mod write;

pub use conflict::UndoConflict;
pub use conflict_policy::UndoConflictPolicy;
pub use plan::UndoPlan;
pub use plan_error::UndoPlanError;
pub use skipped_file::UndoSkippedFile;
pub use write::UndoWrite;
