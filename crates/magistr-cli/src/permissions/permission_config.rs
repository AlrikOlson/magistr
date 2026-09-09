//! Full permission configuration for a project.

use serde::{Deserialize, Serialize};

use super::audit_retention::AuditRetention;
use super::auto_allow_rule::AutoAllowRule;
use super::noninteractive_policy::NoninteractivePolicy;
use super::permission_profile::PermissionProfile;
use crate::error::{MagistrError, MapMagistrErr, Result};

/// Full permission configuration for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// The permission profile to use.
    #[serde(default = "default_profile")]
    pub profile: PermissionProfile,

    /// Handling of unanswered approvals when no approval interface is attached.
    #[serde(default)]
    pub noninteractive: NoninteractivePolicy,

    /// Custom auto-allow rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<AutoAllowRule>,

    /// Audit log retention bounds ([retention] in the TOML).
    #[serde(default, skip_serializing_if = "AuditRetention::is_unlimited")]
    pub retention: AuditRetention,
}

fn default_profile() -> PermissionProfile {
    PermissionProfile::Standard
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            profile: PermissionProfile::Standard,
            noninteractive: NoninteractivePolicy::Refuse,
            rules: Vec::new(),
            retention: AuditRetention::default(),
        }
    }
}

impl PermissionConfig {
    /// Create a config with a specific profile and no custom rules.
    pub fn with_profile(profile: PermissionProfile) -> Self {
        Self {
            profile,
            ..Self::default()
        }
    }

    /// Add a custom auto-allow rule.
    pub fn add_rule(&mut self, rule: AutoAllowRule) {
        self.rules.push(rule);
    }

    /// Serialize to TOML.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).with_context(MagistrError::Config)
    }

    /// Deserialize from TOML.
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).with_context(MagistrError::Config)
    }
}
