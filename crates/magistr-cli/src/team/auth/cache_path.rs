//! Where verified identities are cached on this machine.

use std::path::PathBuf;

/// The per-user magistr state directory, `~/.magistr` by default.
const HOME_DIR_NAME: &str = ".magistr";
/// Overrides the whole state directory — the variable is the directory
/// itself, GNUPGHOME-style, so tests and sandboxes never touch a real home.
const HOME_ENV: &str = "MAGISTR_HOME";
/// The cache file inside it.
const IDENTITY_FILE: &str = "identity.json";

/// `~/.magistr/identity.json` — the verified-identity cache.
///
/// `None` when neither `MAGISTR_HOME` nor `HOME`/`USERPROFILE` is set,
/// which means this machine has nowhere to cache a login; callers treat
/// that as "no verified identity" rather than as an error.
#[must_use]
pub fn cache_path() -> Option<PathBuf> {
    home_dir().map(|dir| dir.join(IDENTITY_FILE))
}

/// The magistr state directory for this user.
fn home_dir() -> Option<PathBuf> {
    let non_empty = |key: &str| {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    if let Some(explicit) = non_empty(HOME_ENV) {
        return Some(PathBuf::from(explicit));
    }
    non_empty("HOME")
        .or_else(|| non_empty("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(HOME_DIR_NAME))
}
