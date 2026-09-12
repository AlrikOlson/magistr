use anyhow::Result;

/// Build the adapter registered under the config's provider, in a given table.
///
/// # Why the table is a parameter
///
/// Two other ways to let a test reach the served path were considered: a
/// `cfg(test)` fake arm in the dispatch, or a fake provider key registered
/// for real. Both were rejected. A `cfg(test)` arm means the path a test
/// executes is not the path that ships. A production-reachable fake key is a
/// way for a typo to point a human's mirror at nothing.
///
/// Taking the TABLE instead has neither cost. There is ONE body — this one —
/// and nothing wraps it for production alone: [`tracker_port_async`] passes
/// [`PROVIDERS`], a test passes a table of its own composition, and nothing
/// else exists. The tested path is byte-identical to the shipped one because
/// it IS the shipped one, and the fake is unreachable in production for the
/// only durable reason — the shipped table does not contain it.
/// `tests/tracker_command_seam.rs` holds that as an executing assertion.
///
/// [`PROVIDERS`]: crate::tracker::registry::PROVIDERS
/// [`tracker_port_async`]: super::tracker_port_async
pub fn build_tracker_port_in(
    registrations: &[crate::tracker::ProviderRegistration],
    config: &crate::tracker::TrackerConfig,
    credential: Option<crate::tracker::credential::Credential>,
) -> Result<Box<dyn crate::tracker::TrackerPort>> {
    let provider = config.provider.as_deref().unwrap_or_default();
    // SEP-414 downstream half: resolved once here, so adding a provider gets
    // trace propagation by calling one builder rather than by remembering to.
    let project = crate::think::config::resolve_project_id();
    // NO PROVIDER IS NAMED IN THIS FILE. The single provider wiring point
    // lives in tracker::registry::PROVIDERS, and the list of known providers in
    // the refusal is rendered from that same table — so the two cannot drift.
    let request = crate::tracker::ProviderBuild {
        target: config.target.as_deref().unwrap_or_default(),
        project: &project,
        credential: credential.as_ref(),
    };
    Ok(crate::tracker::registry::build_in(
        registrations,
        provider,
        &request,
    )?)
}
