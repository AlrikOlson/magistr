//! The supervision rail's construction-time seed: derived once as data
//! ([`supervision_seed`]) and applied to a model ([`seed_supervision`]). The
//! two change together — a host that opens a project off the surface's
//! thread sends the data, every other entry point seeds its model directly.

use std::path::Path;

use magistr_cockpit::cockpit::{Budget, CharterOffer, CharterRule, RuleMode, SupervisionSeed};
use magistr_engine::cost::BudgetConfig;
use magistr_engine::policy::{PolicyEffect, PolicyRule};

use crate::permissions::{CharterStore, PermissionProfile, ProfileAction, RiskLevel};

/// What [`seed_supervision`] seeds, as data a host can send to the surface.
#[must_use]
pub fn supervision_seed(
    project_dir: &Path,
    profile: PermissionProfile,
    workflow: bool,
    inputs: &crate::config::inputs::ProjectInputs,
) -> SupervisionSeed {
    derive(
        project_dir,
        crate::cost::costs_path::costs_path(inputs).as_deref(),
        profile,
        workflow,
        requested_limit(inputs),
    )
}

fn derive(
    project_dir: &Path,
    ledger: Option<&Path>,
    profile: PermissionProfile,
    workflow: bool,
    requested: Option<f64>,
) -> SupervisionSeed {
    let engine = crate::team::TeamConfig::governance_for(project_dir).compile();
    let mut charter = if engine.rules.is_empty() {
        default_charter(profile, workflow)
    } else {
        engine.rules.iter().map(row).collect()
    };
    // The reader's own charter: an unreadable file seeds nothing, the
    // way the policy runs without it.
    let reader = CharterStore::at(project_dir).load().unwrap_or_default();
    charter.extend(reader.delegations.iter().map(row));
    let offers = reader
        .offers()
        .into_iter()
        .map(|offer| CharterOffer::new(offer.key, offer.subject, offer.allowed, offer.asked))
        .collect();
    SupervisionSeed {
        charter,
        offers,
        budget: governed(project_dir, ledger, requested),
        mode: mode_word(profile, workflow),
    }
}

/// The developer's own limit: the flag or user-config limit the host
/// bound (or the `MAGISTR_BUDGET_REQUESTED` export a child inherited) as
/// captured in the inputs, else the captured user config (an entry point
/// that decided no budget of its own).
fn requested_limit(inputs: &crate::config::inputs::ProjectInputs) -> Option<f64> {
    inputs.budget_requested().or_else(|| {
        inputs
            .user_config()
            .ok()
            .and_then(|cfg| cfg.value.budget.limit)
            .filter(|limit| limit.is_finite() && *limit > 0.0)
    })
}

/// The rail's budget from the governor itself: the roster's policy
/// over the ledger, tightened onto the requested limit by its `min()`.
fn governed(project_dir: &Path, ledger: Option<&Path>, requested: Option<f64>) -> Option<Budget> {
    let local = requested.map_or_else(BudgetConfig::none, BudgetConfig::with_limit);
    let resolved = match ledger {
        Some(ledger) => {
            crate::cost::governed_budget(project_dir, ledger, &local, chrono::Utc::now())
        }
        None => magistr_engine::cost::BudgetPolicy::default()
            .resolve(&local, &magistr_engine::cost::ScopeSpend::new()),
    };
    budget_from(
        resolved.effective.limit,
        requested,
        resolved.binding.is_some(),
    )
}

/// The rail's budget from the governor's answer: `effective` is the
/// cap the session runs under, `requested` the developer's own limit,
/// `bound` whether a scope produced the effective cap. The tick appears
/// exactly when a scope bound tighter than the requested limit.
fn budget_from(effective: Option<f64>, requested: Option<f64>, bound: bool) -> Option<Budget> {
    let effective = effective.filter(|cap| cap.is_finite() && *cap > 0.0)?;
    match requested {
        Some(requested) if bound && requested > effective => {
            Budget::capped(cents(requested)).map(|budget| budget.stop_at(cents(effective)))
        }
        _ => Budget::capped(cents(effective)),
    }
}

fn cents(dollars: f64) -> u64 {
    (dollars * 100.0).round() as u64
}

/// A rule as a charter row. Its one-line description is the reader's
/// words for it; the stable id is the fallback the audit log already
/// names it by.
fn row(rule: &PolicyRule) -> CharterRule {
    let subject = if rule.description.is_empty() {
        &rule.id
    } else {
        &rule.description
    };
    CharterRule::new(subject.as_str(), mode(rule.effect))
}

/// The charter of a project with no governance rule: the policy's own
/// defaults as three rows.
fn default_charter(profile: PermissionProfile, workflow: bool) -> Vec<CharterRule> {
    [
        ("Edit", RiskLevel::Medium),
        ("Run tests", RiskLevel::Low),
        ("git push", RiskLevel::High),
    ]
    .into_iter()
    .map(|(subject, risk)| CharterRule::new(subject, tier_mode(profile, workflow, risk)))
    .collect()
}

/// The badge a risk tier wears under `profile`: AUTO wherever the
/// profile allows without asking, and everywhere under workflow mode,
/// which auto-approves the standard development operations by kind.
fn tier_mode(profile: PermissionProfile, workflow: bool, risk: RiskLevel) -> RuleMode {
    if workflow {
        return RuleMode::Auto;
    }
    match profile.default_action(risk) {
        ProfileAction::Allow => RuleMode::Auto,
        ProfileAction::Ask => RuleMode::Ask,
    }
}

/// The mode word the agent block names after the model.
fn mode_word(profile: PermissionProfile, workflow: bool) -> String {
    if workflow {
        return "Workflow mode".to_owned();
    }
    let profile = match profile {
        PermissionProfile::Cautious => "Cautious",
        PermissionProfile::Standard => "Standard",
        PermissionProfile::Trusted => "Trusted",
        PermissionProfile::Yolo => "Yolo",
    };
    format!("{profile} mode")
}

/// The badge a policy effect wears — `RequireApproval` asks, `Allow`
/// and `Warn` both act without stopping (a warn is
/// permitted but recorded), `Deny` is off.
const fn mode(effect: PolicyEffect) -> RuleMode {
    match effect {
        PolicyEffect::RequireApproval => RuleMode::Ask,
        PolicyEffect::Allow | PolicyEffect::Warn => RuleMode::Auto,
        PolicyEffect::Deny => RuleMode::Off,
    }
}

#[cfg(test)]
#[path = "seed_supervision.tests.rs"]
mod tests;
