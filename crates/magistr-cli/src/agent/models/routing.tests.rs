use crate::in_env as env_test;

use super::*;
#[allow(unused_imports)]
use crate::agent::models::config::{ModelConfig, Provider};
use crate::agent::models::registry::ModelRegistry;

// ── TaskComplexity ─────────────────────────────────────────

#[test]
fn complexity_labels() {
    assert_eq!(TaskComplexity::Simple.label(), "simple");
    assert_eq!(TaskComplexity::Moderate.label(), "moderate");
    assert_eq!(TaskComplexity::Complex.label(), "complex");
}

#[test]
fn complexity_display() {
    assert_eq!(format!("{}", TaskComplexity::Simple), "simple");
    assert_eq!(format!("{}", TaskComplexity::Complex), "complex");
}

#[test]
fn complexity_ordering() {
    assert!(TaskComplexity::Simple < TaskComplexity::Moderate);
    assert!(TaskComplexity::Moderate < TaskComplexity::Complex);
}

#[test]
fn default_model_ids() {
    assert_eq!(
        TaskComplexity::Simple.default_model_id(),
        "claude-haiku-4-5"
    );
    assert_eq!(
        TaskComplexity::Moderate.default_model_id(),
        "claude-sonnet-5"
    );
    assert_eq!(TaskComplexity::Complex.default_model_id(), "claude-opus-5");
}

#[test]
fn every_tier_resolves_against_the_builtin_registry() {
    // A tier id that isn't in the registry silently drops routing to the
    // "first available model of any provider" branch — catch that here.
    let reg = ModelRegistry::defaults();
    for complexity in [
        TaskComplexity::Simple,
        TaskComplexity::Moderate,
        TaskComplexity::Complex,
    ] {
        let id = complexity.default_model_id();
        assert!(
            reg.get(id).is_some(),
            "{complexity} tier id {id} not in registry"
        );
    }
}

#[test]
fn fable_is_never_chosen_by_automatic_routing() {
    // Fable 5 is in the registry but out of every fallback chain by design.
    let reg = ModelRegistry::defaults();
    for prompt in [
        "explain this function",
        "implement a login form and write tests",
        "refactor the entire monorepo and redesign the architecture",
    ] {
        let decision = route(prompt, Some(15), None, &reg);
        assert_ne!(
            decision.model_id, "claude-fable-5-1",
            "routing escalated to Fable 5 for: {prompt}"
        );
    }
}

// ── estimate_complexity ────────────────────────────────────

#[test]
fn short_explain_prompt_is_simple() {
    let est = estimate_complexity("explain this function", None);
    assert_eq!(est.complexity, TaskComplexity::Simple);
    assert!(est.confidence > 0.0);
}

#[test]
fn short_read_prompt_is_simple() {
    let est = estimate_complexity("read the config", None);
    assert_eq!(est.complexity, TaskComplexity::Simple);
}

#[test]
fn fix_typo_is_simple() {
    let est = estimate_complexity("fix the typo in main.rs", None);
    assert_eq!(est.complexity, TaskComplexity::Simple);
}

#[test]
fn refactor_keyword_increases_complexity() {
    let est = estimate_complexity("refactor the authentication module", None);
    assert!(est.complexity >= TaskComplexity::Moderate);
}

#[test]
fn architect_keyword_is_complex() {
    let est = estimate_complexity("architect a new microservice system from scratch", None);
    assert_eq!(est.complexity, TaskComplexity::Complex);
}

#[test]
fn long_prompt_increases_complexity() {
    let words: Vec<&str> = std::iter::repeat_n("implement", 60).collect();
    let prompt = words.join(" ");
    let est = estimate_complexity(&prompt, None);
    assert!(est.complexity >= TaskComplexity::Moderate);
}

#[test]
fn many_files_increases_complexity() {
    let est = estimate_complexity("update the handlers", Some(8));
    assert!(est.complexity >= TaskComplexity::Moderate);
}

#[test]
fn few_files_decreases_complexity() {
    let est = estimate_complexity("update something", Some(1));
    assert_eq!(est.complexity, TaskComplexity::Simple);
}

#[test]
fn empty_prompt_is_simple() {
    let est = estimate_complexity("", None);
    assert_eq!(est.complexity, TaskComplexity::Simple);
}

#[test]
fn confidence_is_bounded() {
    let est = estimate_complexity("explain this", None);
    assert!(est.confidence >= 0.0);
    assert!(est.confidence <= 1.0);

    let est2 = estimate_complexity("refactor the entire monorepo architecture", Some(20));
    assert!(est2.confidence >= 0.0);
    assert!(est2.confidence <= 1.0);
}

#[test]
fn reasoning_is_not_empty_for_keyword_match() {
    let est = estimate_complexity("explain this function", None);
    assert!(!est.reasoning.is_empty());
    assert!(est.reasoning.contains("simple keywords"));
}

#[test]
fn mixed_signals_tend_toward_moderate() {
    // "explain" (simple) + many files (complex signal)
    let est = estimate_complexity("explain the architecture", Some(7));
    // Should be moderate or complex, not simple
    assert!(est.complexity >= TaskComplexity::Moderate);
}

// ── route ──────────────────────────────────────────────────

#[test]
fn user_override_takes_priority() {
    let reg = ModelRegistry::defaults();
    let decision = route("explain this", None, Some("claude-fable-5-1"), &reg);
    assert!(decision.is_override);
    assert_eq!(decision.model_id, "claude-fable-5-1");
    assert!(decision.complexity.is_none());
}

#[test]
fn user_override_unknown_model_passes_through() {
    let reg = ModelRegistry::defaults();
    let decision = route("hello", None, Some("my-custom-model"), &reg);
    assert!(decision.is_override);
    assert_eq!(decision.model_id, "my-custom-model");
}

#[test]
fn simple_task_routes_to_cheapest_available() {
    let reg = ModelRegistry::defaults();
    let decision = route("explain this function", None, None, &reg);
    assert!(!decision.is_override);
    assert!(decision.complexity.is_some());
    // The exact model depends on which are "available" (API keys set),
    // but the complexity should be Simple
    let est = decision.complexity.as_ref().unwrap();
    assert_eq!(est.complexity, TaskComplexity::Simple);
}

#[test]
fn complex_task_routes_to_most_capable() {
    let reg = ModelRegistry::defaults();
    let decision = route(
        "refactor the entire monorepo and redesign the architecture",
        Some(15),
        None,
        &reg,
    );
    assert!(!decision.is_override);
    let est = decision.complexity.as_ref().unwrap();
    assert_eq!(est.complexity, TaskComplexity::Complex);
}

#[test]
fn route_with_no_available_models_returns_ideal() {
    if !env_test::in_env(&[("ANTHROPIC_API_KEY", None)]) {
        return;
    }
    // Registry with no available models (all need keys that aren't set)
    let reg = ModelRegistry::defaults();
    assert!(
        reg.available().is_empty(),
        "the fixture supplies no credentials"
    );
    let decision = route("hello", None, None, &reg);
    assert_eq!(decision.model_id, TaskComplexity::Simple.default_model_id());
    assert_eq!(
        decision.reason,
        "no models available — routing to ideal model anyway"
    );
}

#[test]
fn route_reason_mentions_complexity() {
    let reg = ModelRegistry::defaults();
    let decision = route("explain what this does", None, None, &reg);
    // Reason should mention the task type
    assert!(
        decision.reason.contains("simple")
            || decision.reason.contains("fallback")
            || decision.reason.contains("no models")
    );
}

// ── RoutingLog ─────────────────────────────────────────────

#[test]
fn empty_log() {
    let log = RoutingLog::new();
    assert!(log.is_empty());
    assert_eq!(log.len(), 0);
    assert!(log.last().is_none());
}

#[test]
fn record_and_query() {
    let mut log = RoutingLog::new();
    let decision = RoutingDecision {
        model_id: "claude-sonnet-5".into(),
        display_name: "Claude Sonnet 5".into(),
        reason: "test".into(),
        is_override: false,
        complexity: None,
    };
    log.record(decision.clone());

    assert_eq!(log.len(), 1);
    assert!(!log.is_empty());
    assert_eq!(log.last(), Some(&decision));
    assert_eq!(log.decisions().len(), 1);
}

#[test]
fn log_accumulates() {
    let mut log = RoutingLog::new();
    for i in 0..5 {
        log.record(RoutingDecision {
            model_id: format!("model-{i}"),
            display_name: format!("Model {i}"),
            reason: "test".into(),
            is_override: false,
            complexity: None,
        });
    }
    assert_eq!(log.len(), 5);
    assert_eq!(log.last().unwrap().model_id, "model-4");
}

#[test]
fn log_default_is_empty() {
    let log = RoutingLog::default();
    assert!(log.is_empty());
}
