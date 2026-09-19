//! Where verified identities are cached on this machine.

use std::path::PathBuf;

use crate::config::inputs::ProjectInputs;

/// The cache file inside the per-user state directory.
const IDENTITY_FILE: &str = "identity.json";

/// `<state_dir>/identity.json` — the verified-identity cache, over the
/// captured state directory (`MAGISTR_HOME`, else `~/.magistr`).
///
/// `None` when the capture located no state directory, which means this
/// machine has nowhere to cache a login; callers treat that as "no
/// verified identity" rather than as an error.
#[must_use]
pub fn cache_path(inputs: &ProjectInputs) -> Option<PathBuf> {
    inputs.state_dir().map(|dir| dir.join(IDENTITY_FILE))
}
