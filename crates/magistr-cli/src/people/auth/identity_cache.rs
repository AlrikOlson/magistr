//! The on-disk cache of verified identities.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::inputs::ProjectInputs;
use crate::error::MagistrError;

use super::cache_path::cache_path;
use super::verified_identity::VerifiedIdentity;

const CACHE_VERSION: u32 = 2;

/// Every provider this machine has logged into, keyed by
/// [`IdentityProvider::key`].
///
/// Several providers can be cached at once — a laptop that belongs to
/// two teams logs into both — so reads name the provider they want and
/// fall back to the newest unexpired login when they do not care.
///
/// [`IdentityProvider::key`]: super::IdentityProvider::key
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredIdentityCache")]
pub struct IdentityCache {
    /// Schema version, currently 2, after mandatory ID-token verification.
    version: u32,
    /// Provider key → the identity it vouched for.
    identities: BTreeMap<String, VerifiedIdentity>,
}

// Keep migration in deserialization so direct library callers and both file
// loaders share the same trust boundary. Missing versions are legacy, never
// implicitly current. Version 1 could label an unsigned ID token as either
// OIDC or GitHub, so none of its identities can survive the verifier upgrade.
#[derive(Deserialize)]
struct StoredIdentityCache {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    identities: BTreeMap<String, VerifiedIdentity>,
}

impl From<StoredIdentityCache> for IdentityCache {
    fn from(stored: StoredIdentityCache) -> Self {
        if stored.version == CACHE_VERSION {
            Self {
                version: CACHE_VERSION,
                identities: stored.identities,
            }
        } else {
            Self::default()
        }
    }
}

impl Default for IdentityCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            identities: BTreeMap::new(),
        }
    }
}

impl IdentityCache {
    /// Load the cache from the captured state directory, treating an
    /// absent file, an unlocated home and unreadable or corrupt contents
    /// alike as an empty cache.
    ///
    /// A cache is derived state: refusing to start because it went bad
    /// would be worse than logging in again.
    #[must_use]
    pub fn load(inputs: &ProjectInputs) -> Self {
        cache_path(inputs)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Load from an explicit path, distinguishing "absent" from "corrupt".
    pub fn load_from(path: &Path) -> Result<Self, MagistrError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(MagistrError::io(path, e)),
        };
        serde_json::from_str(&text)
            .map_err(|e| MagistrError::parse("json", path.display().to_string(), e))
    }

    /// Write the cache to the captured state directory.
    pub fn save(&self, inputs: &ProjectInputs) -> Result<(), MagistrError> {
        let path = cache_path(inputs).ok_or_else(|| {
            MagistrError::Other(
                "cannot cache a login: neither MAGISTR_HOME nor HOME is set".to_string(),
            )
        })?;
        self.save_to(&path)
    }

    /// Write the cache to an explicit path, creating its directory.
    pub fn save_to(&self, path: &Path) -> Result<(), MagistrError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
        }
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        std::fs::write(path, text).map_err(|e| MagistrError::io(path, e))
    }

    /// Store one login, replacing any earlier one for the same provider.
    /// Deserialization already discarded all identities from older schemas.
    pub fn put(&mut self, identity: VerifiedIdentity) {
        self.identities.insert(identity.provider.clone(), identity);
    }

    /// Forget one provider's login; `true` when there was one.
    pub fn remove(&mut self, provider: &str) -> bool {
        self.identities.remove(provider).is_some()
    }

    /// Forget every login, returning how many there were.
    pub fn clear(&mut self) -> usize {
        let count = self.identities.len();
        self.identities.clear();
        count
    }

    /// The identity to resolve with as of `now`.
    ///
    /// `preferred` — the provider the team requires — wins outright, and
    /// an expired login for it does *not* silently fall through to some
    /// other provider: a team that demands Okta gets Okta or nothing.
    /// With no preference, the newest unexpired login wins.
    #[must_use]
    pub fn select(&self, preferred: Option<&str>, now: DateTime<Utc>) -> Option<&VerifiedIdentity> {
        if let Some(key) = preferred {
            return self.identities.get(key).filter(|id| !id.is_expired(now));
        }
        self.identities
            .values()
            .filter(|id| !id.is_expired(now))
            .max_by_key(|id| id.issued_at)
    }

    /// Every cached login, expired ones included, for `magistr login --status`.
    pub fn entries(&self) -> impl Iterator<Item = &VerifiedIdentity> {
        self.identities.values()
    }

    /// Is nothing cached?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}

#[cfg(test)]
#[path = "identity_cache.tests.rs"]
mod tests;
