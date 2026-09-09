//! Resolve which model a session may actually run on.

use std::path::Path;

use crate::error::MagistrError;
use crate::team::TeamConfig;

/// The model a session in `dir` runs on, given what the caller asked for.
///
/// `None` in means the caller named no model, so the agent's own default
/// stands — the policy has nothing to judge. Otherwise the request goes
/// through [`GovernancePolicy::route_model`]: an approved model comes
/// back unchanged, a refused one is routed to the policy's fallback, and
/// a refused one with no viable fallback is an error rather than a
/// silent downgrade.
///
/// [`GovernancePolicy::route_model`]: magistr_engine::policy::GovernancePolicy::route_model
pub fn governed_model(dir: &Path, requested: Option<&str>) -> Result<Option<String>, MagistrError> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    let policy = TeamConfig::governance_for(dir);
    match policy.route_model(requested) {
        Some(model) => Ok(Some(model.to_string())),
        None => Err(MagistrError::Other(format!(
            "model '{requested}' is not approved by this team's policy \
             (.magistr/team.json) and the policy names no usable fallback — \
             run `magistr policy show` to see the approved models"
        ))),
    }
}

#[cfg(test)]
#[path = "governed_model.tests.rs"]
mod tests;
