//! Permission profile controlling how tool calls are authorized.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::profile_action::ProfileAction;
use super::risk_level::RiskLevel;

/// Permission profile controlling how tool calls are authorized.
///
/// Profiles range from most restrictive (`Cautious`) to least (`Yolo`).
/// Each profile defines default behavior for each risk level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionProfile {
    /// Ask for everything — suitable for untrusted repos.
    Cautious,
    /// Auto-allow reads and searches, ask for writes and commands.
    Standard,
    /// Auto-allow most operations, ask only for destructive actions.
    Trusted,
    /// Auto-allow everything — for personal projects with no risk.
    Yolo,
}

impl PermissionProfile {
    /// All available profiles, in order from most to least restrictive.
    pub fn all() -> &'static [PermissionProfile] {
        &[
            PermissionProfile::Cautious,
            PermissionProfile::Standard,
            PermissionProfile::Trusted,
            PermissionProfile::Yolo,
        ]
    }

    /// Human-readable description of this profile.
    pub fn description(self) -> &'static str {
        match self {
            Self::Cautious => "Ask for everything (untrusted repos)",
            Self::Standard => "Auto-allow reads, ask for writes and commands",
            Self::Trusted => "Auto-allow most, ask only for destructive actions",
            Self::Yolo => "Auto-allow everything (personal projects)",
        }
    }

    /// Default action for a given risk level under this profile.
    pub fn default_action(self, risk: RiskLevel) -> ProfileAction {
        match (self, risk) {
            // Cautious: ask for everything
            (Self::Cautious, _) => ProfileAction::Ask,

            // Standard: allow low risk, ask for medium and high
            (Self::Standard, RiskLevel::Low) => ProfileAction::Allow,
            (Self::Standard, RiskLevel::Medium | RiskLevel::High) => ProfileAction::Ask,

            // Trusted: allow low and medium, ask only for high
            (Self::Trusted, RiskLevel::Low | RiskLevel::Medium) => ProfileAction::Allow,
            (Self::Trusted, RiskLevel::High) => ProfileAction::Ask,

            // Yolo: allow everything
            (Self::Yolo, _) => ProfileAction::Allow,
        }
    }
}

impl fmt::Display for PermissionProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cautious => write!(f, "cautious"),
            Self::Standard => write!(f, "standard"),
            Self::Trusted => write!(f, "trusted"),
            Self::Yolo => write!(f, "yolo"),
        }
    }
}

impl std::str::FromStr for PermissionProfile {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cautious" => Ok(Self::Cautious),
            "standard" => Ok(Self::Standard),
            "trusted" => Ok(Self::Trusted),
            "yolo" => Ok(Self::Yolo),
            other => Err(format!(
                "unknown permission profile: '{other}' (expected: cautious, standard, trusted, yolo)"
            )),
        }
    }
}
