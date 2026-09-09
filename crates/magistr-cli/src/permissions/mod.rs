//! Smart permission management — profiles, risk levels, and auto-allow rules.
//!
//! Provides four permission profiles (cautious → yolo), risk-level classification
//! for tool calls, pattern-based auto-allow rules, and an audit log.

// ── Core types ─────────────────────────────────────────────────────
pub mod charter_store;
pub mod classify_risk;
pub mod cockpit_policy;
pub mod decision_reason;
pub mod default_policy;
pub mod delegation_scope;
pub mod evaluate;
pub mod governed_model;
pub mod high_risk_bash_patterns;
pub mod permission_decision;
pub mod permission_profile;
pub mod policy;
pub mod policy_request_for;
pub mod profile_action;
pub mod resolve_against;
pub mod risk_level;

// ── Rules ──────────────────────────────────────────────────────────
pub mod auto_allow_rule;
pub mod rule_matcher;
pub mod rules_from_quality_gates;

// ── Config ─────────────────────────────────────────────────────────
pub mod config_path;
pub mod load_config;
pub mod permission_config;
pub mod save_config;

// ── Audit ──────────────────────────────────────────────────────────
pub mod append_entry;
pub mod apply_retention;
pub mod audit_entry;
pub mod audit_log_path;
pub mod audit_retention;
pub mod audit_summary;
pub mod export_csv;
pub mod load_entries;
pub mod suggest_trust_upgrade;
pub mod summarize;
pub mod violation_report;

// ── Backward-compat shims (host test files) ────────────────────────
pub mod audit;
pub mod config;
pub mod rules;

// ── Re-exports ─────────────────────────────────────────────────────
#[allow(unused_imports)]
pub use append_entry::append_entry;
#[allow(unused_imports)]
pub use apply_retention::apply_retention;
#[allow(unused_imports)]
pub use audit_entry::AuditEntry;
#[allow(unused_imports)]
pub use audit_log_path::audit_log_path;
#[allow(unused_imports)]
pub use audit_retention::AuditRetention;
#[allow(unused_imports)]
pub use audit_summary::AuditSummary;
#[allow(unused_imports)]
pub use auto_allow_rule::AutoAllowRule;
#[allow(unused_imports)]
pub use charter_store::CharterStore;
#[allow(unused_imports)]
pub use classify_risk::classify_risk;
#[allow(unused_imports)]
pub use cockpit_policy::CockpitPolicy;
#[allow(unused_imports)]
pub use config_path::config_path;
#[allow(unused_imports)]
pub use decision_reason::DecisionReason;
#[allow(unused_imports)]
pub use default_policy::DefaultPermissionPolicy;
#[allow(unused_imports)]
pub use delegation_scope::DelegationScope;
#[allow(unused_imports)]
pub use evaluate::evaluate;
#[allow(unused_imports)]
pub use export_csv::export_csv;
#[allow(unused_imports)]
pub use governed_model::governed_model;
#[allow(unused_imports)]
pub use high_risk_bash_patterns::HIGH_RISK_BASH_PATTERNS;
#[allow(unused_imports)]
pub use load_config::load_config;
#[allow(unused_imports)]
pub use load_entries::load_entries;
#[allow(unused_imports)]
pub use permission_config::PermissionConfig;
#[allow(unused_imports)]
pub use permission_decision::PermissionDecision;
#[allow(unused_imports)]
pub use permission_profile::PermissionProfile;
#[allow(unused_imports)]
pub use policy::PermissionPolicy;
#[allow(unused_imports)]
pub use policy_request_for::policy_request_for;
#[allow(unused_imports)]
pub use profile_action::ProfileAction;
#[allow(unused_imports)]
pub use resolve_against::resolve_against;
#[allow(unused_imports)]
pub use risk_level::RiskLevel;
#[allow(unused_imports)]
pub use rule_matcher::RuleMatcher;
#[allow(unused_imports)]
pub use rules_from_quality_gates::rules_from_quality_gates;
#[allow(unused_imports)]
pub use save_config::save_config;
#[allow(unused_imports)]
pub use suggest_trust_upgrade::suggest_trust_upgrade;
#[allow(unused_imports)]
pub use summarize::summarize;
#[allow(unused_imports)]
pub use violation_report::ViolationReport;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
