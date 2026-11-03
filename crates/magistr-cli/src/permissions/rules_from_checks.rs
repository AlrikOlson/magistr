//! Generate default auto-allow rules from check commands.

use super::auto_allow_rule::AutoAllowRule;

/// Generate default auto-allow rules from check commands.
///
/// Creates Bash prefix rules for each non-empty check command,
/// so checks run without prompting.
pub fn rules_from_checks(check: &str, test: &str, lint: &str) -> Vec<AutoAllowRule> {
    let mut rules = Vec::new();

    for (cmd, label) in [(check, "check"), (test, "test"), (lint, "lint")] {
        let cmd = cmd.trim();
        if !cmd.is_empty() {
            rules.push(AutoAllowRule::bash_prefix(cmd, format!("check: {label}")));
        }
    }

    rules
}
