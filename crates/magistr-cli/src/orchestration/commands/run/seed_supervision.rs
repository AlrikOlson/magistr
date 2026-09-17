//! The supervision rail's construction-time seed.

use std::path::Path;

use magistr_cockpit::cockpit::{Budget, CharterOffer, CharterRule, Model, RuleMode};
use magistr_engine::cost::BudgetConfig;
use magistr_engine::policy::{PolicyEffect, PolicyRule};

use crate::permissions::{CharterStore, PermissionProfile, ProfileAction, RiskLevel};

/// Populate the supervision rail from the session's policy and budget.
///
/// Governance rules provide the charter rows. Without explicit rules, rows
/// show the permission profile's defaults for editing, running tests, and
/// pushing changes. User charter rules and supported delegation offers follow.
///
/// The budget combines team policy, recorded spend, and the requested limit.
/// When policy imposes a lower cap, the meter marks it within the requested
/// range. The mode label is workflow mode or the selected permission profile.
///
/// Usage and timing arrive later through [`AgentEvent::Usage`] and turn events.
///
/// [`AgentEvent::Usage`]: magistr_application::sessions::AgentEvent::Usage
pub fn seed_supervision(
    model: Model,
    project_dir: &Path,
    profile: PermissionProfile,
    workflow: bool,
    inputs: &crate::config::inputs::ProjectInputs,
) -> Model {
    seed_with(
        model,
        project_dir,
        profile,
        workflow,
        requested_limit(inputs),
    )
}

/// [`seed_supervision`] with the developer's requested limit given
/// rather than read — the seam the tests bind through.
fn seed_with(
    mut model: Model,
    project_dir: &Path,
    profile: PermissionProfile,
    workflow: bool,
    requested: Option<f64>,
) -> Model {
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
    let budget = governed(project_dir, requested);
    let rail = model.rail_mut();
    rail.set_charter(charter);
    rail.set_offers(offers);
    rail.set_budget(budget);
    rail.set_mode(mode_word(profile, workflow));
    model
}

/// The developer's own limit: the flag or user-config limit `main`
/// exported as `MAGISTR_BUDGET_REQUESTED`, else the captured user config
/// (an entry point that skipped `main`'s export).
fn requested_limit(inputs: &crate::config::inputs::ProjectInputs) -> Option<f64> {
    dollars_in("MAGISTR_BUDGET_REQUESTED").or_else(|| {
        inputs
            .user_config()
            .ok()
            .and_then(|cfg| cfg.value.budget.limit)
            .filter(|limit| limit.is_finite() && *limit > 0.0)
    })
}

/// The rail's budget from the governor itself: the roster's policy
/// over the ledger, tightened onto the requested limit by its `min()`.
fn governed(project_dir: &Path, requested: Option<f64>) -> Option<Budget> {
    let local = requested.map_or_else(BudgetConfig::none, BudgetConfig::with_limit);
    let resolved = match crate::cost::costs_path::costs_path() {
        Some(ledger) => {
            crate::cost::governed_budget(project_dir, &ledger, &local, chrono::Utc::now())
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

/// A positive, finite dollar amount exported under `name`, or nothing.
fn dollars_in(name: &str) -> Option<f64> {
    std::env::var(name)
        .ok()
        .and_then(|dollars| dollars.parse::<f64>().ok())
        .filter(|dollars| dollars.is_finite() && *dollars > 0.0)
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
