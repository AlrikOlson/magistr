//! Authorize, apply and audit one edit of `.magistr/team.json`.

use std::path::Path;

use crate::error::MagistrError;
use crate::permissions::AuditRetention;
use crate::team::auth::require_verified;
use crate::team::{
    Capability, Identity, RosterChange, TeamConfig, TeamRole, is_ci_env, resolve_role,
    roster_access,
};

use super::audit_roster_edit;

/// Load the roster, decide whether the caller may make `change`, apply
/// it, save, and record the outcome in the audit log.
///
/// Authorize a roster edit: identity is
/// resolved from `dir`, [`require_verified`] runs ahead of the role so a
/// team that demands a provider gets one, and [`roster_access`] answers
/// the capability question. A refusal is audited too — an attempt to
/// grant oneself Owner is the event the log exists for.
///
/// `change` describes the edit in role terms once the roster is loaded
/// (it needs the current role of the member being altered); `op` is the
/// [`TeamConfig`] mutation itself, which keeps its own integrity rules
/// (duplicate id, unknown id, the last Owner).
///
/// Returns the saved roster.
pub fn edit_roster(
    dir: &Path,
    member_id: &str,
    change: impl FnOnce(&TeamConfig) -> Result<RosterChange, MagistrError>,
    op: impl FnOnce(&mut TeamConfig) -> Result<(), MagistrError>,
    audit_log: Option<&Path>,
    retention: &AuditRetention,
) -> Result<TeamConfig, MagistrError> {
    let mut config = TeamConfig::load(dir)?.ok_or_else(|| {
        MagistrError::Other("no .magistr/team.json — run `magistr team init` first".to_string())
    })?;

    let identity = Identity::resolve(dir);
    // Ahead of the role, as in `attach` and the handoff: a role resolved
    // from an identity nobody vouched for is no basis for a roster edit.
    require_verified(Some(&config), &identity)?;
    let role = resolve_role(Some(&config), &identity, is_ci_env());

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
fn refusal(role: TeamRole, change: RosterChange) -> String {
    if change.touches_owner() && role.can(Capability::ManageTeam) {
        format!("role {role} may not grant or alter Owner — only an Owner can transfer ownership")
    } else {
        format!("role {role} may not edit the roster — Admin or Owner required")
    }
}

#[cfg(test)]
#[path = "edit_roster.tests.rs"]
mod tests;
