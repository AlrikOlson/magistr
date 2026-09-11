use serde::{Deserialize, Serialize};

/// Precedence of workspace configuration layers, from lowest to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigLayer {
    Defaults,
    OrgPolicy,
    User,
    Workspace,
    Session,
}

impl ConfigLayer {
    pub const ORDER: [Self; 5] = [
        Self::Defaults,
        Self::OrgPolicy,
        Self::User,
        Self::Workspace,
        Self::Session,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defaults => "defaults",
            Self::OrgPolicy => "org-policy",
            Self::User => "user",
            Self::Workspace => "workspace",
            Self::Session => "session",
        }
    }
}

impl std::fmt::Display for ConfigLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
