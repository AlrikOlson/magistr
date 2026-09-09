//! Generate default auto-allow rules from quality gate commands.

use super::auto_allow_rule::AutoAllowRule;

/// Generate default auto-allow rules from quality gate commands.
///
/// Creates Bash prefix rules for each non-empty quality gate command,
/// so quality gates run without prompting.
pub fn rules_from_quality_gates(check: &str, test: &str, lint: &str) -> Vec<AutoAllowRule> {
    let mut rules = Vec::new();

    for (cmd, label) in [(check, "check"), (test, "test"), (lint, "lint")] {
        let cmd = cmd.trim();
        if !cmd.is_empty() {
            rules.push(AutoAllowRule::bash_prefix(
                cmd,
                format!("quality gate: {label}"),
            ));
        }
    }

    rules
}
