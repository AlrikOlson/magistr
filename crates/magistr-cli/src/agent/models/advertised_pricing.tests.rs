//! Tests for pricing lookup of agent-advertised model ids.

use super::*;

fn registry() -> ModelRegistry {
    ModelRegistry::defaults()
}

// ── concrete ids resolve ────────────────────────────────────────────────────

#[test]
fn concrete_id_resolves_to_its_pricing() {
    let reg = registry();
    let entry = advertised_pricing("claude-fable-5-1", &reg).expect("fable priced");
    assert_eq!(entry.config.id, "claude-fable-5-1");
    assert!((entry.config.cost_per_million_input - 10.0).abs() < f64::EPSILON);
}

#[test]
fn context_suffix_is_stripped_before_lookup() {
    // What Claude Code 2.1.258 actually advertises for Fable.
    let reg = registry();
    let entry = advertised_pricing("claude-fable-5-1[1m]", &reg).expect("fable priced");
    assert_eq!(entry.config.id, "claude-fable-5-1");
}

// ── aliases stay unpriced ───────────────────────────────────────────────────

#[test]
fn bare_aliases_are_not_priced() {
    // The agent's SDK resolves these to a concrete model we cannot see.
    // Guessing a price here is the bug this function exists to prevent.
    let reg = registry();
    for alias in ["sonnet", "opus", "haiku", "default", "opusplan"] {
        assert!(
            advertised_pricing(alias, &reg).is_none(),
            "alias {alias} must not be priced"
        );
    }
}

#[test]
fn aliases_with_a_context_suffix_are_still_not_priced() {
    assert!(advertised_pricing("opus[1m]", &registry()).is_none());
}

#[test]
fn unknown_ids_are_not_priced() {
    assert!(advertised_pricing("some-future-model", &registry()).is_none());
}

// ── suffix stripping edge cases ─────────────────────────────────────────────

#[test]
fn an_unclosed_bracket_is_not_treated_as_a_suffix() {
    assert_eq!(
        strip_context_suffix("claude-fable-5[1m"),
        "claude-fable-5[1m"
    );
}

#[test]
fn an_id_without_a_bracket_is_unchanged() {
    assert_eq!(strip_context_suffix("claude-opus-5"), "claude-opus-5");
}
