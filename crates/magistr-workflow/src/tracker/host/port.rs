use anyhow::Result;

use super::build_tracker_port_in;

/// The port for the configured provider, resolved from the shipped
/// registration table, for callers already inside a runtime — the served
/// `tracker_setup` tool.
pub(crate) async fn tracker_port_async(
    config: &crate::tracker::TrackerConfig,
) -> Result<Box<dyn crate::tracker::TrackerPort>> {
    tracker_port_async_in(crate::tracker::registry::PROVIDERS, config).await
}

/// [`tracker_port_async`] against an arbitrary registration table. Unknown
/// providers are named rather than silently ignored — a typo in a provider
/// must not look like "nothing to do".
pub async fn tracker_port_async_in(
    registrations: &[crate::tracker::ProviderRegistration],
    config: &crate::tracker::TrackerConfig,
) -> Result<Box<dyn crate::tracker::TrackerPort>> {
    let provider = config.provider.as_deref().unwrap_or_default();
    build_tracker_port_in(
        registrations,
        config,
        tracker_credential_async(provider).await,
    )
}

/// Stored credentials, with the OAuth refresh profile a stored credential
/// carries registered on the resolver so an expired token refreshes instead
/// of dying with "no OAuth configuration is registered to refresh it".
fn credential_resolver() -> crate::tracker::credential::Resolver {
    use crate::tracker::credential::CredentialStore as _;

    let data_dir = crate::infra::PersistenceConfig::from_env().data_dir;
    let store = std::sync::Arc::new(crate::tracker::credential::FileCredentialStore::new(
        &data_dir,
    ));
    let mut resolver = crate::tracker::credential::Resolver::new(store.clone());
    for provider in ["linear", "jira"] {
        if let Ok(Some(stored)) = store.load(provider)
            && let Some(config) =
                crate::tracker::credential::authcode::stored_refresh_profile(&stored)
        {
            resolver = resolver.with_oauth(provider, config);
        }
    }
    resolver
}

/// Resolve a usable credential, preferring the stored one and falling back to
/// the environment. `None` when neither is present, so callers can say
/// something useful instead of failing obscurely.
///
/// Async because every caller is already inside a runtime; building a
/// runtime here would panic with "Cannot start a runtime from within a
/// runtime", which is exactly how the retired sync twin once shipped a live
/// panic.
async fn tracker_credential_async(
    provider: &str,
) -> Option<crate::tracker::credential::Credential> {
    use crate::tracker::credential::{CredentialPort, CredentialStore};

    if let Ok(c) = credential_resolver().credential(provider).await {
        return Some(c);
    }
    // Documented fallback.
    let env = crate::tracker::credential::EnvCredentialStore;
    env.load(provider).ok().flatten().map(|s| s.as_credential())
}
