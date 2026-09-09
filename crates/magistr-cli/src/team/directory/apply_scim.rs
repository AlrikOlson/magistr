//! Apply a SCIM request to the roster.

use crate::error::MagistrError;
use crate::team::{TeamConfig, TeamMember, TeamRole};

use super::group_role::GroupRole;
use super::scim_effect::ScimEffect;
use super::scim_operation::ScimOperation;
use super::scim_outcome::ScimOutcome;
use super::scim_request::ScimRequest;
use super::scim_user::ScimUser;

/// Apply `request` to `config`, returning what it did.
///
/// This is provisioning without a hand edit: the roster is written from
/// what the directory pushed, the role comes from the group mapping,
/// and the caller's only job afterwards is [`TeamConfig::save`].
///
/// The last Owner is protected here exactly as it is against a human
/// editor — a directory that deletes, deactivates or demotes the only
/// remaining Owner is refused. An IdP misconfiguration should not be
/// able to lock a team out of its own roster.
///
/// # Errors
///
/// On a create for somebody already provisioned, an operation against a
/// member the roster does not carry, a rename onto an id already taken,
/// or a change that would leave no active Owner.
pub fn apply_scim(
    config: &mut TeamConfig,
    request: &ScimRequest,
) -> Result<ScimOutcome, MagistrError> {
    let id = request.member_id().to_string();
    if id.is_empty() {
        return Err(bad_request("the request names no member"));
    }
    match request {
        ScimRequest::Create { user } => create(config, user),
        ScimRequest::Replace { user, .. } => replace(config, &id, user),
        ScimRequest::Patch { operations, .. } => patch(config, &id, operations),
        ScimRequest::Delete { .. } => {
            let role = role_of(config, &id)?;
            config.remove_member(&id)?;
            Ok(ScimOutcome::new(id, ScimEffect::Removed, role))
        }
    }
}

/// `POST /Users`.
fn create(config: &mut TeamConfig, user: &ScimUser) -> Result<ScimOutcome, MagistrError> {
    let id = user.member_id().to_string();
    let role = role_for(config, &user.group_names());
    // A provider that re-adds somebody it deprovisioned sends a POST,
    // not a PATCH — there is nothing on its side to patch. Reviving the
    // existing row is the only reading that keeps their history; a
    // create over an *active* member is the uniqueness conflict RFC
    // 7644 §3.3 makes a 409.
    if let Some(existing) = config.row(&id) {
        if existing.active {
            return Err(bad_request(&format!(
                "'{id}' is already provisioned — a create over an active member is a \
                 uniqueness conflict"
            )));
        }
        set_member(config, &id, user.display(), role, true)?;
        return Ok(ScimOutcome::new(id, ScimEffect::Reactivated, role));
    }
    config.add_member(TeamMember {
        id: id.clone(),
        name: user.display(),
        role,
        active: user.active,
    })?;
    Ok(ScimOutcome::new(id, ScimEffect::Created, role))
}

/// `PUT /Users/{id}`.
fn replace(
    config: &mut TeamConfig,
    id: &str,
    user: &ScimUser,
) -> Result<ScimOutcome, MagistrError> {
    let before = row_snapshot(config, id)?;
    let role = role_for(config, &user.group_names());
    let renamed = user.member_id();
    if !renamed.is_empty() && !renamed.eq_ignore_ascii_case(id) {
        rename(config, id, renamed)?;
    }
    let id = if renamed.is_empty() { id } else { renamed };
    let after = (user.display(), role, user.active);
    set_member(config, id, after.0.clone(), role, user.active)?;
    Ok(ScimOutcome::new(id, effect_of(&before, &after), role))
}

/// `PATCH /Users/{id}` — the path both Okta and Entra deprovision over.
fn patch(
    config: &mut TeamConfig,
    id: &str,
    operations: &[ScimOperation],
) -> Result<ScimOutcome, MagistrError> {
    let before = row_snapshot(config, id)?;
    let mut id = id.to_string();
    let (mut name, mut role, mut active) = before.clone();
    let mut ignored = Vec::new();
    let mut renamed: Option<String> = None;
    for operation in operations {
        for (path, value) in operation.targets() {
            match normalize(&path).as_str() {
                "active" => active = value.as_bool().unwrap_or(!operation.is_remove()),
                "username" => renamed = value.as_str().map(str::to_string),
                "displayname" | "name.formatted" => {
                    name = value.as_str().map(str::to_string);
                }
                "name" => {
                    name = serde_json::from_value(value.clone())
                        .ok()
                        .and_then(|parsed: super::scim_name::ScimName| parsed.display())
                }
                "groups" => role = role_for(config, &groups_in(value)),
                _ => ignored.push(path),
            }
        }
    }
    if let Some(new_id) = renamed.filter(|new| !new.trim().is_empty() && *new != id) {
        rename(config, &id, &new_id)?;
        id = new_id;
    }
    let after = (name, role, active);
    set_member(config, &id, after.0.clone(), role, active)?;
    Ok(ScimOutcome::new(id, effect_of(&before, &after), role).ignoring(ignored))
}

/// The role a set of directory group names earns; the roster's own
/// fallback when nothing maps.
fn role_for(config: &TeamConfig, groups: &[String]) -> TeamRole {
    GroupRole::highest(&config.directory.group_roles, groups).unwrap_or(config.default_role)
}

/// The group names inside a SCIM `groups` value.
fn groups_in(value: &serde_json::Value) -> Vec<String> {
    serde_json::from_value::<Vec<super::scim_value::ScimValue>>(value.clone())
        .map(|values| {
            values
                .iter()
                .filter_map(super::scim_value::ScimValue::label)
                .map(super::ldap_config::LdapConfig::group_name)
                .collect()
        })
        .unwrap_or_default()
}

/// An attribute path, stripped of its schema URN and case-folded.
fn normalize(path: &str) -> String {
    path.rsplit(':')
        .next()
        .unwrap_or(path)
        .trim()
        .to_lowercase()
}

/// What a member's row says now — the three things the roster records
/// and an effect is judged against.
type Snapshot = (Option<String>, TeamRole, bool);

fn row_snapshot(config: &TeamConfig, id: &str) -> Result<Snapshot, MagistrError> {
    config
        .row(id)
        .map(|row| (row.name.clone(), row.role, row.active))
        .ok_or_else(|| bad_request(&format!("no member '{id}' on the roster")))
}

fn role_of(config: &TeamConfig, id: &str) -> Result<TeamRole, MagistrError> {
    row_snapshot(config, id).map(|(_, role, _)| role)
}

/// Write a member's name, role and provisioned state, refusing a change
/// that would leave the team with no active Owner.
fn set_member(
    config: &mut TeamConfig,
    id: &str,
    name: Option<String>,
    role: TeamRole,
    active: bool,
) -> Result<(), MagistrError> {
    ensure_owner_remains(config, id, role, active)?;
    let row = config
        .row_mut(id)
        .ok_or_else(|| bad_request(&format!("no member '{id}' on the roster")))?;
    row.name = name;
    row.role = role;
    row.active = active;
    Ok(())
}

/// Move a member to a new id, refusing a collision.
fn rename(config: &mut TeamConfig, id: &str, new_id: &str) -> Result<(), MagistrError> {
    if config.row(new_id).is_some() {
        return Err(bad_request(&format!(
            "cannot rename '{id}' to '{new_id}': that id is already on the roster"
        )));
    }
    let row = config
        .row_mut(id)
        .ok_or_else(|| bad_request(&format!("no member '{id}' on the roster")))?;
    row.id = new_id.to_string();
    Ok(())
}

/// Refuse a change that would take the last active Owner away.
fn ensure_owner_remains(
    config: &TeamConfig,
    id: &str,
    role: TeamRole,
    active: bool,
) -> Result<(), MagistrError> {
    let stays_owner = role == TeamRole::Owner && active;
    if stays_owner {
        return Ok(());
    }
    let others = config
        .members
        .iter()
        .filter(|m| m.active && m.role == TeamRole::Owner && !m.id.eq_ignore_ascii_case(id))
        .count();
    if others > 0 {
        return Ok(());
    }
    let was_owner = config
        .row(id)
        .is_some_and(|row| row.active && row.role == TeamRole::Owner);
    if was_owner {
        return Err(MagistrError::Permission(format!(
            "the directory would leave the team with no active Owner by changing '{id}' \
             — refused; promote another Owner first"
        )));
    }
    Ok(())
}

/// Which effect a transition amounts to.
///
/// Provisioned state outranks everything: a request that both demotes
/// somebody and deactivates them is a deactivation, because that is the
/// half an operator must see.
fn effect_of(before: &Snapshot, after: &Snapshot) -> ScimEffect {
    match (before.2, after.2) {
        (true, false) => ScimEffect::Deactivated,
        (false, true) => ScimEffect::Reactivated,
        _ if before.1 != after.1 || before.0 != after.0 => ScimEffect::Updated,
        _ => ScimEffect::Unchanged,
    }
}

fn bad_request(why: &str) -> MagistrError {
    MagistrError::Other(format!("SCIM request refused: {why}"))
}

#[cfg(test)]
#[path = "apply_scim.tests.rs"]
mod tests;
