use super::TargetPhase;
use anyhow::Result;
pub async fn setup_probe(
    port: &dyn crate::tracker::TrackerPort,
    display_name: &str,
    may_create: bool,
) -> Result<TargetPhase> {
    use crate::tracker::TrackerError;

    match port.probe_target().await {
        Ok(_) => Ok(TargetPhase::Present),
        Err(TrackerError::NotFound(_)) => {
            if !may_create {
                return Ok(TargetPhase::MissingAndNotCreated);
            }
            port.create_target(display_name).await.map_err(|e| {
                // A create failure is very often a permission one, and the
                // provider's own words are the only useful thing to say.
                anyhow::anyhow!(
                    "could not create the destination: {e}\n\n\
                     If that was a permission error: a personal API key acts \
                     as you, so you must be allowed to create one yourself. \
                     On the OAuth path (`tracker sign-in`) the token also \
                     needs the provider's admin scope — the default scopes \
                     cannot provision."
                )
            })?;
            Ok(TargetPhase::Created)
        }
        // "Cannot check" must not read as "missing". A provider that declines to
        // introspect is not evidence of a problem, and refusing to continue
        // would be worse than the bug this probe was added to fix.
        Err(TrackerError::Unsupported(_)) => Ok(TargetPhase::Unverifiable),
        Err(e) => Err(anyhow::anyhow!("could not check the destination: {e}")),
    }
}
