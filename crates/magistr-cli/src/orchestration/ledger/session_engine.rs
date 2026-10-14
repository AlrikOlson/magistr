//! The engine handle a cockpit session checkpoints against.

use std::path::Path;

use magistr_engine::{BudgetConfig, EngineHandle, NativeReader, WorkflowConfig};

/// An [`EngineHandle`] for the cockpit session in `root`, attached to a
/// reader over the project's native workflow engines.
///
/// Capture validates roadmap progress and the selected chunk at the owner.
/// The loop driver owns iteration policy independently of this checkpoint handle.
/// Native attachment performs no read; evidence operations validate their
/// required owner when requested.
#[must_use]
pub fn session_engine(root: &Path, native: NativeReader) -> EngineHandle {
    let id = root.file_name().map_or_else(
        || "session".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    EngineHandle::new(id, WorkflowConfig::default(), BudgetConfig::default()).with_native(native)
}
