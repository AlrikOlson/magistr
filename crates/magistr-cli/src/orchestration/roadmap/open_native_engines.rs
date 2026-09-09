//! Open the in-process think-and-ship engines for a project directory when
//! its `.magistr/project.json` selects the native roadmap backend.
//!
//! The flag is read as plain JSON (no stack re-detection side effects), so
//! this is safe to call from any command. Call it BEFORE entering raw mode:
//! `RoadmapEngine::with_persistence` and the reasoning server print load
//! banners on stderr, which would corrupt the alternate screen.

use std::path::Path;

use magistr_engine::{NativeEngines, RoadmapBackend, TasConfig};

/// The engines for `dir`, or `None` when the project runs the YAML roadmap
/// (or has no `.magistr/project.json`). Store location and persistence come
/// from the process environment (`THINK_AND_SHIP_*`).
#[must_use]
pub fn open_native_engines(dir: &Path) -> Option<NativeEngines> {
    open_native_engines_with(dir, |key| std::env::var(key).ok())
}

/// Same as [`open_native_engines`] with an injected environment lookup, so
/// tests can point the store at a temp directory.
#[must_use]
pub(crate) fn open_native_engines_with(
    dir: &Path,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<NativeEngines> {
    if !roadmap_backend_of(dir).is_native() {
        return None;
    }
    Some(NativeEngines::open(&TasConfig::from_lookup(dir, lookup)))
}

/// The backend a project effectively runs: the declared `roadmap_backend`,
/// or the inference for projects that predate the key
/// (`magistr_core::roadmap::backend_for`).
#[must_use]
pub(crate) fn roadmap_backend_of(dir: &Path) -> RoadmapBackend {
    magistr_core::roadmap::backend_for(dir)
}

#[cfg(test)]
#[path = "open_native_engines.tests.rs"]
mod tests;
