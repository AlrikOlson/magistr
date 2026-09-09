//! Detect patterns that suggest upgrading the trust level.

use super::audit_entry::AuditEntry;
use super::risk_level::RiskLevel;

/// Detect patterns that suggest upgrading the trust level.
///
/// Returns a human-readable suggestion if the user has approved many
/// tool calls that would be auto-approved at a higher trust level.
pub fn suggest_trust_upgrade(entries: &[AuditEntry]) -> Option<String> {
    let session_allowed = entries
        .iter()
        .filter(|e| e.allowed && e.reason == "profile default")
        .count();

    if session_allowed >= 20 {
        // Check if most were low-risk
        let low_risk_count = entries
            .iter()
            .filter(|e| e.allowed && e.risk == RiskLevel::Low && e.reason == "profile default")
            .count();

        if low_risk_count > session_allowed / 2 {
            return Some(format!(
                "You've manually approved {session_allowed} tool calls this session \
                 ({low_risk_count} were low-risk). Consider upgrading to --trust standard."
            ));
        }
    }

    if session_allowed >= 40 {
        let medium_or_less = entries
            .iter()
            .filter(|e| e.allowed && e.risk != RiskLevel::High && e.reason == "profile default")
            .count();

        if medium_or_less > session_allowed * 3 / 4 {
            return Some(format!(
                "You've manually approved {session_allowed} tool calls ({medium_or_less} non-destructive). \
                 Consider upgrading to --trust trusted."
            ));
        }
    }

    None
}
