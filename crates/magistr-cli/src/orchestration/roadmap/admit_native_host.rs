//! Admit this process as the host of a project whose `.magistr/project.json`
//! selects the native roadmap backend, and open its writer.
//!
//! The loops and the cockpit mutate the store — gates recorded, chunks
//! started and completed, blockers set — so they take the host lease before
//! raw mode. One host per project and store: a second `next-loop` or cockpit
//! on the same project is refused here with the lease's own reason, while
//! the agent's `magistr mcp-serve` is the other role and coexists.

use std::path::Path;

use color_eyre::eyre;
use magistr_core::evidence::writer_config_from_env;
use magistr_engine::{NativeConfig, NativeEngines, ProjectAuthority, WriterRole};

use super::open_native_reader::roadmap_backend_of;

/// The admitted writer for `dir`, `None` when the project runs the YAML
/// roadmap (or has no `.magistr/project.json`). Store location and
/// persistence come from the process environment (`THINK_AND_SHIP_*`); the
/// writer carries the project's evidence capability, so the gates it
/// records are bound to the source and its completions are governed.
///
/// # Errors
/// The host lease is held by another cockpit or loop on this project and
/// store, or its lock file cannot be taken.
pub fn admit_native_host(dir: &Path) -> eyre::Result<Option<NativeEngines>> {
    admit_with_configuration(dir, || writer_config_from_env(dir))
}

/// Same as [`admit_native_host`] with an injected environment lookup, so
/// tests can point the store at a temp directory.
#[cfg(test)]
pub(crate) fn admit_native_host_with(
    dir: &Path,
    lookup: impl Fn(&str) -> Option<String>,
) -> eyre::Result<Option<NativeEngines>> {
    admit_with_configuration(dir, || magistr_core::evidence::writer_config(dir, lookup))
}

fn admit_with_configuration(
    dir: &Path,
    configure: impl FnOnce() -> NativeConfig,
) -> eyre::Result<Option<NativeEngines>> {
    if !roadmap_backend_of(dir).is_native() {
        return Ok(None);
    }
    let authority = ProjectAuthority::admit(&configure(), WriterRole::Host)
        .map_err(|error| eyre::eyre!("{error}"))?;
    Ok(Some(authority.open()))
}
