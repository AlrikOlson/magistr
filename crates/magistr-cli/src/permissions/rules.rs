//! Auto-allow rules — re-exports from split submodules.

pub use super::auto_allow_rule::AutoAllowRule;
pub use super::rule_matcher::RuleMatcher;
pub use super::rules_from_checks::rules_from_checks;

#[cfg(test)]
#[path = "rules.tests.rs"]
mod tests;
