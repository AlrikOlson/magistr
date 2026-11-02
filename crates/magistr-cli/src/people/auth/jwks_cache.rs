//! A bounded per-flow key cache. Trust never survives a failed refresh.
use crate::error::MagistrError;
use jsonwebtoken::jwk::JwkSet;
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 64;
const MAX_AGE: Duration = Duration::from_secs(300);
const MISS_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(super) struct JwksCache {
    keys: Option<(JwkSet, Instant, Duration)>,
    last_refresh: Option<Instant>,
    last_miss_refresh: Option<Instant>,
}

impl JwksCache {
    pub(super) fn keys_for(
        &mut self,
        kid: &str,
        now: Instant,
        fetch: impl FnOnce() -> Result<(JwkSet, Duration), MagistrError>,
    ) -> Result<&JwkSet, MagistrError> {
        let fresh = self
            .keys
            .as_ref()
            .is_some_and(|(_, at, ttl)| now.saturating_duration_since(*at) < *ttl);
        let known = self
            .keys
            .as_ref()
            .is_some_and(|(keys, _, _)| keys.find(kid).is_some());
        if !(fresh && known) {
            // A key rotation can refresh a fresh cache once per cooldown.
            // Repeated random kids or outages cannot trigger a request storm.
            let recent_miss = self
                .last_miss_refresh
                .is_some_and(|at| now.saturating_duration_since(at) < MISS_COOLDOWN);
            let recent_failure = self.keys.is_none()
                && self
                    .last_refresh
                    .is_some_and(|at| now.saturating_duration_since(at) < MISS_COOLDOWN);
            if (!known && recent_miss) || recent_failure {
                return Err(MagistrError::Other(
                    "OIDC signing key unavailable; refresh is rate limited".into(),
                ));
            }
            if !known && self.keys.is_some() {
                self.last_miss_refresh = Some(now);
            }
            self.keys = None;
            self.last_refresh = Some(now);
            let (keys, ttl) = fetch()?;
            if keys.keys.is_empty() || keys.keys.len() > MAX_KEYS {
                return Err(MagistrError::Other(
                    "OIDC JWKS must contain between 1 and 64 keys".into(),
                ));
            }
            self.keys = Some((keys, now, ttl.min(MAX_AGE)));
        }
        let keys = &self
            .keys
            .as_ref()
            .ok_or_else(|| MagistrError::Other("OIDC signing keys unavailable".into()))?
            .0;
        if keys.find(kid).is_none() {
            self.last_miss_refresh = Some(now);
            return Err(MagistrError::Other(
                "OIDC token has an unknown signing key ID".into(),
            ));
        }
        Ok(keys)
    }
}

#[cfg(test)]
#[path = "jwks_cache.tests.rs"]
mod tests;
