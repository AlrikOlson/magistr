//! Publish the owner endpoint for a host this process admitted.
//!
//! The loops hold the project's host writer for as long as they run; the
//! endpoint is how a cockpit opened in another terminal acts through that
//! writer instead of being refused. It lives exactly as long as the handle
//! this returns — keep it beside the loop and let it drop before the
//! writer does.

use color_eyre::eyre;
use magistr_engine::{NativeEngines, OwnerEndpoint};

/// The endpoint for `engines`, `None` when the project has no native
/// writer (a YAML roadmap).
///
/// # Errors
/// The endpoint cannot be served: another listener is live at the
/// owner's socket (a previous owner's runtime still winding down), or the
/// socket cannot be bound. Loud on purpose — a host that cannot be
/// attached to would leave a second cockpit with nothing to explain the
/// refusal.
pub async fn serve_owner_endpoint(
    engines: Option<&NativeEngines>,
) -> eyre::Result<Option<OwnerEndpoint>> {
    let Some(engines) = engines else {
        return Ok(None);
    };
    OwnerEndpoint::serve(engines.clone())
        .await
        .map(Some)
        .map_err(|error| eyre::eyre!("{error}"))
}
