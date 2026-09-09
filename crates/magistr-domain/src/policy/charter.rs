use alloc::{format, string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use super::delegation_offer::DelegationOffer;
use super::policy_condition::PolicyCondition;
use super::policy_decision::PolicyDecision;
use super::policy_effect::PolicyEffect;
use super::policy_request::PolicyRequest;
use super::policy_rule::PolicyRule;
use super::scope_tally::ScopeTally;

/// The reader's own standing orders for a project — the policy write
/// API under the roster, and the track record it is widened from.
///
/// Two halves. `delegations` are real [`PolicyRule`]s the reader wrote
/// by answering asks (*always allow* under a scope, *never allow*, or a
/// written condition to ask again); [`apply`] lets a matching one
/// replace the local profile's answer — an `Allow` lowers an ask, a
/// `Deny` or `RequireApproval` raises one. `tallies` count how the
/// reader has answered under every scope, and [`offers`] are the scopes
/// with a clean record past [`OFFER_AFTER`], each accepted by
/// [`accept`] into an `Allow` delegation.
///
/// What this can never do is pass the roster. A charter widens only
/// the *local* decision, and the org's [`PolicyEngine::tighten`] folds
/// its maximum over that afterwards, exactly as it folds over the
/// profile — a delegated `Allow` under a roster `Deny` is a `Deny` by
/// arithmetic, with no branch here that could forget it.
///
/// [`apply`]: Charter::apply
/// [`offers`]: Charter::offers
/// [`accept`]: Charter::accept
/// [`OFFER_AFTER`]: Charter::OFFER_AFTER
/// [`PolicyEngine::tighten`]: super::policy_engine::PolicyEngine::tighten
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Charter {
    /// The rules the reader wrote, newest last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delegations: Vec<PolicyRule>,
    /// How the reader answered, per scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tallies: Vec<ScopeTally>,
}

impl Charter {
    /// Allows in a row, with no deny, before a scope is offered.
    pub const OFFER_AFTER: u32 = 5;

    /// Whether the charter holds anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.delegations.is_empty() && self.tallies.is_empty()
    }

    /// The id the `Allow` delegation for `key` carries.
    #[must_use]
    pub fn delegation_id(key: &str) -> String {
        format!("delegate:{key}")
    }

    /// Write `rule` into the charter, replacing one with the same id.
    /// `true` when the charter changed.
    pub fn delegate(&mut self, rule: PolicyRule) -> bool {
        match self.delegations.iter_mut().find(|r| r.id == rule.id) {
            Some(existing) if *existing == rule => false,
            Some(existing) => {
                *existing = rule;
                true
            }
            None => {
                self.delegations.push(rule);
                true
            }
        }
    }

    /// Take the rule `id` back out. `true` when one was there.
    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.delegations.len();
        self.delegations.retain(|r| r.id != id);
        self.delegations.len() != before
    }

    /// The delegation with `id`, if the reader wrote one.
    #[must_use]
    pub fn delegation(&self, id: &str) -> Option<&PolicyRule> {
        self.delegations.iter().find(|r| r.id == id)
    }

    /// The local decision with the reader's delegations applied: the
    /// tightest matching delegation replaces `local` and names itself
    /// as `binding`; with none matching, `local` stands unnamed.
    ///
    /// Tightest-wins among the delegations for the same reason the
    /// roster folds a maximum — a *never allow* the reader wrote beats
    /// an *always allow* they wrote earlier, whatever order the two
    /// were written in.
    #[must_use]
    pub fn apply(&self, local: PolicyEffect, request: &PolicyRequest) -> PolicyDecision {
        let matches: Vec<&PolicyRule> = self
            .delegations
            .iter()
            .filter(|rule| rule.matches(request))
            .collect();
        let Some(effect) = matches.iter().map(|rule| rule.effect).max() else {
            return PolicyDecision {
                effect: local,
                ..PolicyDecision::default()
            };
        };
        PolicyDecision {
            effect,
            matched: matches.iter().map(|rule| rule.id.clone()).collect(),
            binding: matches
                .iter()
                .find(|rule| rule.effect == effect)
                .map(|rule| rule.id.clone()),
            override_allowed: false,
        }
    }

    /// Count one answer under the scope `key` (created with `subject`
    /// and `condition` on its first answer).
    pub fn record(
        &mut self,
        key: &str,
        subject: &str,
        condition: &PolicyCondition,
        allowed: bool,
    ) -> &ScopeTally {
        let index = match self.tallies.iter().position(|t| t.key == key) {
            Some(index) => index,
            None => {
                self.tallies
                    .push(ScopeTally::new(key, subject, condition.clone()));
                self.tallies.len() - 1
            }
        };
        let tally = &mut self.tallies[index];
        tally.note(allowed);
        tally
    }

    /// The record under `key`, if any ask was answered there.
    #[must_use]
    pub fn tally(&self, key: &str) -> Option<&ScopeTally> {
        self.tallies.iter().find(|t| t.key == key)
    }

    /// The widenings the record supports: every scope the reader has
    /// allowed at least [`OFFER_AFTER`](Self::OFFER_AFTER) times and
    /// never denied, that no delegation already covers.
    #[must_use]
    pub fn offers(&self) -> Vec<DelegationOffer> {
        self.tallies
            .iter()
            .filter(|t| t.is_clean() && t.allowed >= Self::OFFER_AFTER)
            .filter(|t| !self.delegations.iter().any(|r| r.condition == t.condition))
            .map(|t| DelegationOffer::new(&t.key, &t.subject, t.allowed, t.asked()))
            .collect()
    }

    /// Accept the offer for `key`: write the `Allow` delegation over
    /// the scope's condition, described in the reader's words, and
    /// return it. `None` for a scope nothing was ever answered under.
    pub fn accept(&mut self, key: &str) -> Option<PolicyRule> {
        let tally = self.tally(key)?;
        let rule = PolicyRule::new(
            Self::delegation_id(key),
            tally.condition.clone(),
            PolicyEffect::Allow,
        )
        .describe(tally.subject.clone());
        self.delegate(rule.clone());
        Some(rule)
    }
}

#[cfg(test)]
#[path = "charter.tests.rs"]
mod tests;
