//! Auto-allow rules — re-exports from split submodules.

pub use super::auto_allow_rule::AutoAllowRule;
pub use super::rule_matcher::RuleMatcher;
pub use super::rules_from_quality_gates::rules_from_quality_gates;

#[cfg(test)]
#[path = "rules.tests.rs"]
mod tests;
