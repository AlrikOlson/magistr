//! Observe the in-process magistr store for a project directory when
//! its `.magistr/project.json` selects the native plan backend.
//!
//! Current settings are read without changing them, so
//! this is safe to call from any command. Reading needs no admission, so a
//! one-shot command answers beside a running cockpit or `magistr mcp`.
//! Call it BEFORE entering raw mode: `PlanEngine::with_persistence` and
//! the reasoning server print load banners on stderr, which would corrupt
//! the alternate screen.

use std::path::Path;

use magistr_engine::{NativeConfig, NativeReader, PlanBackend};

/// The reader for `dir`, or `None` when the project runs the YAML plan
/// store. Store location and persistence come
/// from the process environment (`MAGISTR_WORKFLOW_*`).
pub fn open_native_reader(dir: &Path) -> crate::error::Result<Option<NativeReader>> {
    open_with_configuration(dir, || NativeConfig::from_env(dir))
}

/// Same as [`open_native_reader`] with an injected environment lookup, so
/// tests can point the store at a temp directory.
#[cfg(test)]
pub(crate) fn open_native_reader_with(
    dir: &Path,
    lookup: impl Fn(&str) -> Option<String>,
) -> crate::error::Result<Option<NativeReader>> {
    open_with_configuration(dir, || NativeConfig::from_lookup(dir, lookup))
}

fn open_with_configuration(
    dir: &Path,
    configure: impl FnOnce() -> NativeConfig,
) -> crate::error::Result<Option<NativeReader>> {
    if !plan_backend_of(dir)?.is_native() {
        return Ok(None);
    }
    Ok(Some(NativeReader::open(&configure())))
}

/// The backend a project effectively runs: the declared `plan_backend`,
/// or the native default (`magistr_core::plan::backend_for`).
pub(crate) fn plan_backend_of(dir: &Path) -> crate::error::Result<PlanBackend> {
    magistr_core::plan::backend_for(dir)
}

#[cfg(test)]
#[path = "open_native_reader.tests.rs"]
mod tests;
