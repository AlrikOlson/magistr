use serde::{Deserialize, Serialize};

use crate::cost::budget_rule::BudgetRule;
use crate::policy::governance_policy::GovernancePolicy;
use crate::policy::policy_engine::PolicyEngine;

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The governance artifact a control plane publishes to its fleet:
/// one generation of the org's [`GovernancePolicy`] and [`BudgetRule`]s,
/// sealed with a digest of its own contents.
///
/// Two fields do the work. `generation` is monotonic, so a node (or an
/// operator) can tell newer from older without a clock, and a registry
/// can refuse a rollback to a looser policy. `digest` is a hash of the
/// content, so "the node reports generation 7" and "the node is running
/// what generation 7 actually says" are different claims — a node that
/// reports the right generation with the wrong digest is drifted, not
/// current.
///
/// The digest is FNV-1a over canonical JSON: **change detection, not a
/// tamper seal**. Anyone who can edit a bundle can recompute its digest.
/// Corruption and staleness are visible to the fleet rollup; bundles
/// are not cryptographically signed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyBundle {
    /// Monotonic version of the published policy. Generation 0 means
    /// nothing has been published yet.
    pub generation: u64,
    /// Digest over `generation`, `policy` and `budgets`.
    pub digest: String,
    /// The governance rules.
    #[serde(default)]
    pub policy: GovernancePolicy,
    /// The spending caps.
    #[serde(default)]
    pub budgets: Vec<BudgetRule>,
}

impl PolicyBundle {
    /// The bundle a fleet starts with: generation 0, no policy, no caps.
    #[must_use]
    pub fn empty() -> Self {
        Self::seal(0, GovernancePolicy::default(), Vec::new())
    }

    /// Seal a generation of policy and budgets, computing its digest.
    #[must_use]
    pub fn seal(generation: u64, policy: GovernancePolicy, budgets: Vec<BudgetRule>) -> Self {
        let digest = Self::digest_of(generation, &policy, &budgets);
        Self {
            generation,
            digest,
            policy,
            budgets,
        }
    }

    /// Whether the digest still matches the contents.
    ///
    /// A bundle that arrives over the wire is checked before it is
    /// published: a mismatch means it was mangled in transit or edited
    /// without resealing, and either way it is refused rather than
    /// applied.
    #[must_use]
    pub fn verify(&self) -> bool {
        self.digest == Self::digest_of(self.generation, &self.policy, &self.budgets)
    }

    /// Whether this bundle is newer than `other`.
    #[must_use]
    pub fn supersedes(&self, other: &Self) -> bool {
        self.generation > other.generation
    }

    /// Whether anything has been published yet.
    #[must_use]
    pub fn is_published(&self) -> bool {
        self.generation > 0
    }

    /// The engine this bundle's policy compiles to.
    #[must_use]
    pub fn engine(&self) -> PolicyEngine {
        self.policy.compile()
    }

    /// The engine a node runs once it applies this bundle on top of its
    /// own local policy.
    ///
    /// [`PolicyEngine::evaluate`] folds `max`
    /// over every *matching* rule, so the union of two rule sets decides
    /// at least as strictly as either set alone. Applying a bundle can
    /// therefore only tighten a node — no merge conflict to resolve, no
    /// precedence for a local config to win.
    #[must_use]
    pub fn merge_local(&self, local: &GovernancePolicy) -> PolicyEngine {
        self.engine().merged_with(&local.compile())
    }

    /// FNV-1a over the canonical JSON of the bundle's contents.
    fn digest_of(generation: u64, policy: &GovernancePolicy, budgets: &[BudgetRule]) -> String {
        // Nothing these types can hold makes serde_json fail today —
        // a non-finite budget limit renders as `null` rather than
        // erroring — but a digest that panics is a control plane that
        // stops publishing, so the fallback is the equally
        // deterministic Debug rendering rather than an unwrap.
        let bytes = serde_json::to_vec(&(generation, policy, budgets))
            .unwrap_or_else(|_| format!("{generation}|{policy:?}|{budgets:?}").into_bytes());

        let mut hash = FNV_OFFSET;
        for byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        format!("fnv1a64:{hash:016x}")
    }
}

impl Default for PolicyBundle {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
#[path = "policy_bundle.tests.rs"]
mod tests;
