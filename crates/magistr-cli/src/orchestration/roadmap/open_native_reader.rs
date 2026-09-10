//! Observe the in-process think-and-ship store for a project directory when
//! its `.magistr/project.json` selects the native roadmap backend.
//!
//! The flag is read as plain JSON (no stack re-detection side effects), so
//! this is safe to call from any command. Reading needs no admission, so a
//! one-shot command answers beside a running cockpit or `magistr mcp-serve`.
//! Call it BEFORE entering raw mode: `RoadmapEngine::with_persistence` and
//! the reasoning server print load banners on stderr, which would corrupt
//! the alternate screen.

use std::path::Path;

use magistr_engine::{NativeReader, RoadmapBackend, TasConfig};

/// The reader for `dir`, or `None` when the project runs the YAML roadmap
/// (or has no `.magistr/project.json`). Store location and persistence come
/// from the process environment (`THINK_AND_SHIP_*`).
#[must_use]
pub fn open_native_reader(dir: &Path) -> Option<NativeReader> {
    open_native_reader_with(dir, |key| std::env::var(key).ok())
}

/// Same as [`open_native_reader`] with an injected environment lookup, so
/// tests can point the store at a temp directory.
#[must_use]
pub(crate) fn open_native_reader_with(
    dir: &Path,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<NativeReader> {
    if !roadmap_backend_of(dir).is_native() {
        return None;
    }
    Some(NativeReader::open(&TasConfig::from_lookup(dir, lookup)))
}

/// The backend a project effectively runs: the declared `roadmap_backend`,
/// or the inference for projects that predate the key
/// (`magistr_core::roadmap::backend_for`).
#[must_use]
pub(crate) fn roadmap_backend_of(dir: &Path) -> RoadmapBackend {
    magistr_core::roadmap::backend_for(dir)
}

#[cfg(test)]
#[path = "open_native_reader.tests.rs"]
mod tests;
