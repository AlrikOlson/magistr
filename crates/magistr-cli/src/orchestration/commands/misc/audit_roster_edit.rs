//! Record a roster edit — allowed or refused — in the permission audit log.

use std::path::Path;

use chrono::Utc;

use crate::permissions::{AuditEntry, AuditRetention, RiskLevel, append_entry, apply_retention};
use crate::team::{Identity, RosterChange, TeamRole};

/// Append one `team_roster` entry for `change` to the audit log at
/// `audit_log`, then apply the retention bounds.
///
/// Best-effort, like the cockpit policy's own logging and the session
/// handoff: a roster edit is never failed because the log could not be
/// written. Unlike the handoff, refusals are recorded too — a denied
/// attempt to grant oneself Owner is precisely the event an audit log
/// exists to keep.
pub fn audit_roster_edit(
    allowed: bool,
    change: RosterChange,
    member_id: &str,
    actor: &Identity,
    role: TeamRole,
    audit_log: Option<&Path>,
    retention: &AuditRetention,
) {
    let Some(log_path) = audit_log else {
        return;
    };

    let verb = match change {
        RosterChange::Add(role) => format!("add '{member_id}' as {role}"),
        RosterChange::Remove(_) => format!("remove '{member_id}'"),
        RosterChange::SetRole { from, to } => {
            format!("change '{member_id}' from {from} to {to}")
        }
    };
    let reason = if allowed {
        format!("{role} applied roster edit: {verb}")
    } else {
        format!("{role} refused roster edit: {verb}")
    };

    let entry = AuditEntry {
        timestamp: Utc::now(),
        tool_name: "team_roster".to_string(),
        risk: if change.touches_owner() {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        },
        allowed,
        reason,
        project: None,
        input_summary: Some(member_id.to_string()),
        actor: Some(actor.id().to_string()),
    };

    if append_entry(log_path, &entry).is_ok() {
        let _ = apply_retention(log_path, retention);
    }
}

#[cfg(test)]
#[path = "audit_roster_edit.tests.rs"]
mod tests;
