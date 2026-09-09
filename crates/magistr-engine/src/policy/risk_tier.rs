use serde::{Deserialize, Serialize};

/// How dangerous a request is, as a policy condition can see it.
///
/// The engine layer is pure domain and cannot depend on the CLI's
/// `permissions::RiskLevel`, so this is the tier a caller maps its own
/// classification onto before evaluating a policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskTier {
    /// Read-only, informational.
    Low,
    /// Reversible file changes.
    Medium,
    /// Potentially destructive or irreversible.
    High,
}

impl RiskTier {
    /// Every tier, least dangerous first.
    pub const ALL: [RiskTier; 3] = [RiskTier::Low, RiskTier::Medium, RiskTier::High];

    /// The serialized name of this tier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RiskTier::Low => "low",
            RiskTier::Medium => "medium",
            RiskTier::High => "high",
        }
    }
}

impl std::fmt::Display for RiskTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RiskTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(RiskTier::Low),
            "medium" | "med" => Ok(RiskTier::Medium),
            "high" => Ok(RiskTier::High),
            other => Err(format!(
                "unknown risk tier '{other}' — valid tiers: low, medium, high"
            )),
        }
    }
}

#[cfg(test)]
#[path = "risk_tier.tests.rs"]
mod tests;
