//! Authorize, apply and audit one edit of `.magistr/people.json`.

use std::path::Path;

use crate::config::inputs::ProjectInputs;
use crate::error::MagistrError;
use crate::people::auth::require_verified;
use crate::people::{
    Capability, Identity, PeopleConfig, PersonRole, RosterChange, resolve_role, roster_access,
};
use crate::permissions::AuditRetention;

use super::audit_roster_edit;

/// Load the roster, decide whether the caller may make `change`, apply
/// it, save, and record the outcome in the audit log.
///
/// Authorize a roster edit: identity is resolved from `dir` and the
/// captured `inputs`, [`require_verified`] runs ahead of the role so a
/// team that demands a provider gets one, and [`roster_access`] answers
/// the capability question. A refusal is audited too — an attempt to
/// grant oneself Owner is the event the log exists for.
///
/// `change` describes the edit in role terms once the roster is loaded
/// (it needs the current role of the member being altered); `op` is the
/// [`PeopleConfig`] mutation itself, which keeps its own integrity rules
/// (duplicate id, unknown id, the last Owner).
///
/// Returns the saved roster.
pub fn edit_roster(
    dir: &Path,
    inputs: &ProjectInputs,
    member_id: &str,
    change: impl FnOnce(&PeopleConfig) -> Result<RosterChange, MagistrError>,
    op: impl FnOnce(&mut PeopleConfig) -> Result<(), MagistrError>,
    audit_log: Option<&Path>,
    retention: &AuditRetention,
) -> Result<PeopleConfig, MagistrError> {
    let mut config = PeopleConfig::load(dir)?.ok_or_else(|| {
        MagistrError::Other("no .magistr/people.json — run `magistr people init` first".to_string())
    })?;

    let identity = Identity::resolve(dir, inputs);
    // Ahead of the role, as in `attach` and the handoff: a role resolved
    // from an identity nobody vouched for is no basis for a roster edit.
    require_verified(Some(&config), &identity)?;
    let role = resolve_role(Some(&config), &identity, inputs.is_ci());

    let change = change(&config)?;

    if !roster_access(role, change) {
        audit_roster_edit(
            false, change, member_id, &identity, role, audit_log, retention,
        );
        return Err(MagistrError::Other(refusal(role, change)));
    }

    op(&mut config)?;
    config.save(dir)?;
    audit_roster_edit(
        true, change, member_id, &identity, role, audit_log, retention,
    );
    Ok(config)
}

/// Why the edit was refused, in the terms the capability matrix uses —
/// which of the two rules the caller fell short of.
fn refusal(role: PersonRole, change: RosterChange) -> String {
    if change.touches_owner() && role.can(Capability::ManageTeam) {
        format!("role {role} may not grant or alter Owner — only an Owner can transfer ownership")
    } else {
        format!("role {role} may not edit the roster — Admin or Owner required")
    }
}

#[cfg(test)]
#[path = "edit_roster.tests.rs"]
mod tests;
