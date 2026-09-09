use serde::{Deserialize, Serialize};

/// A model's cost class, ordered cheapest first.
///
/// The ordering lets a roster express "nothing above Standard on this
/// project" as one comparison instead of enumerating every model id it
/// has never heard of — which matters because model catalogues change
/// faster than roster files do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    /// Small, cheap models.
    Economy,
    /// The everyday working tier.
    Standard,
    /// The most capable and most expensive tier.
    Frontier,
}

impl ModelTier {
    /// Every tier, cheapest first.
    pub const ALL: [ModelTier; 3] = [ModelTier::Economy, ModelTier::Standard, ModelTier::Frontier];

    /// The serialized name of this tier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ModelTier::Economy => "economy",
            ModelTier::Standard => "standard",
            ModelTier::Frontier => "frontier",
        }
    }
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ModelTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "economy" | "cheap" => Ok(ModelTier::Economy),
            "standard" => Ok(ModelTier::Standard),
            "frontier" | "premium" => Ok(ModelTier::Frontier),
            other => Err(format!(
                "unknown model tier '{other}' — valid tiers: economy, standard, frontier"
            )),
        }
    }
}

#[cfg(test)]
#[path = "model_tier.tests.rs"]
mod tests;
