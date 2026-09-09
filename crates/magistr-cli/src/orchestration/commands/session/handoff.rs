//! Session handoff: transfer a session to a new owner.

use std::path::Path;

use chrono::Utc;

use crate::error::{MagistrError, Result};
use crate::permissions::{AuditEntry, AuditRetention, RiskLevel, append_entry, apply_retention};
use crate::session;
use crate::team::{Identity, SessionAction, TeamRole, session_access};

/// Transfer `session_id` to `new_owner`, gated by the session-access
/// matrix: the current owner may hand off their own session, and roles
/// with the manage-team capability (Admin+) may take over any.
///
/// A successful transfer is recorded in the permission audit log at
/// `audit_log` (best-effort, like the cockpit policy's own logging)
/// with the acting identity as the audit actor, and the audit retention
/// bounds are applied after the append. Returns a human-readable
/// receipt.
pub fn handoff_session(
    conn: &rusqlite::Connection,
    session_id: &str,
    new_owner: &str,
    actor: &Identity,
    role: TeamRole,
    audit_log: Option<&Path>,
    retention: &AuditRetention,
) -> Result<String> {
    let record = session::get_session(conn, session_id)?
        .ok_or_else(|| MagistrError::Other(format!("session {session_id} not found")))?;

    let owns = record
        .owner
        .as_deref()
        .is_some_and(|o| o.eq_ignore_ascii_case(actor.id()));

    if !session_access(role, SessionAction::TakeOver, owns) {
        return Err(MagistrError::Other(format!(
            "role {role} may not hand off a session owned by {} — \
             only the session owner or Admin+ can transfer it",
            record.owner.as_deref().unwrap_or("nobody"),
        )));
    }

    session::set_session_owner(conn, session_id, new_owner)?;

    let entry = AuditEntry {
        timestamp: Utc::now(),
        tool_name: "session_handoff".to_string(),
        risk: RiskLevel::Medium,
        allowed: true,
        reason: format!(
            "session handed off from {} to {new_owner}",
            record.owner.as_deref().unwrap_or("(unowned)"),
        ),
        project: Some(record.project.clone()),
        input_summary: Some(session_id.to_string()),
        actor: Some(actor.id().to_string()),
    };
    if let Some(log_path) = audit_log
        && append_entry(log_path, &entry).is_ok()
    {
        let _ = apply_retention(log_path, retention);
    }

    Ok(format!("Session {session_id} handed off to {new_owner}"))
}

#[cfg(test)]
#[path = "handoff.tests.rs"]
mod tests;
