//! Native edit, files and undo commands. Every verb resolves intent into
//! transaction changes and runs them only through the shared c6 pipeline;
//! no command writes, renames or unlinks a workspace file itself.
mod diff;
mod edit_input;
mod edit_op;
mod edit_rename;
mod edit_resolve;
mod edit_rewrite;
mod files_input;
mod files_op;
mod files_resolve;
mod gate;
mod move_imports;
mod move_imports_c;
mod move_imports_java;
mod move_imports_js;
mod move_imports_python;
mod resolution;
mod resolved;
pub mod rust_move;
mod service;
mod source_reads;
mod transaction_errors;
mod undo_input;
mod undo_row;
mod undo_run;
mod undo_scope;
mod write_action;
mod write_report;

pub use edit_input::EditInput;
pub use edit_op::EditOp;
pub use files_input::FilesInput;
pub use files_op::FilesOp;
pub use service::WorkspaceCommands;
pub use undo_input::UndoInput;
pub use undo_row::UndoRow;
pub use undo_scope::UndoScope;
pub use write_action::WriteAction;
pub use write_report::WriteReport;

/// Ops beyond this many changed lines are confirm-gated (retained §9.4 rule).
pub(super) const CONFIRM_LINE_LIMIT: usize = 200;
/// Ceiling on the aggregated dry-run diff a CONFIRM_REQUIRED answer carries.
pub(super) const CONFIRM_DIFF_CHARS: usize = 8000;
