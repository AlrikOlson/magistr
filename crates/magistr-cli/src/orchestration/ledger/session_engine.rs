//! The engine handle a cockpit session checkpoints against.

use std::path::Path;

use magistr_engine::{BudgetConfig, EngineHandle, NativeReader, WorkflowConfig};

/// An [`EngineHandle`] for the cockpit session in `root`, attached to a
/// reader over the project's native workflow engines.
///
/// Capture reads the roadmap cursor from the native store: the chunk in
/// progress, or the next ready chunk. The cockpit's phase and iteration are
/// held separately in `Model` and are not synchronized to this handle.
/// Snapshots therefore contain the repository and roadmap cursor, with
/// workflow fields at their engine defaults.
#[must_use]
pub fn session_engine(root: &Path, native: NativeReader) -> EngineHandle {
    let id = root.file_name().map_or_else(
        || "session".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    EngineHandle::new(id, WorkflowConfig::default(), BudgetConfig::default()).with_native(native)
}
