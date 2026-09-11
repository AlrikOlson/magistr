use crate::agent::Backend;

/// The model list Claude Code 2.1 actually advertises: a mix of bare
/// aliases and one concrete id carrying a context-window suffix.
fn probed_claude() -> crate::agent::probe::ProviderModelSet {
    use crate::agent::probe::{ProbeModel, ProviderModelSet};
    ProviderModelSet {
        provider_id: "claude".into(),
        label: "Claude Code".into(),
        models: [
            ("default", "Default (recommended)"),
            ("sonnet", "Sonnet"),
            ("opus[1m]", "Opus (1M context)"),
            ("claude-fable-5-1[1m]", "Fable"),
        ]
        .iter()
        .map(|(id, name)| ProbeModel {
            id: (*id).into(),
            name: (*name).into(),
        })
        .collect(),
        probed: true,
    }
}

fn models_payload() -> serde_json::Value {
    let registry = crate::models::ModelRegistry::defaults();
    super::cmd_models::build_models_json(&registry, &[probed_claude()])
}

#[test]
fn json_lists_what_the_agent_advertises_not_the_registry() {
    // The old command printed the built-in registry as though it were the
    // available models, so it claimed models the agent would have rejected.
    let payload = models_payload();
    let ids: Vec<&str> = payload["providers"][0]["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["default", "sonnet", "opus[1m]", "claude-fable-5-1[1m]"]
    );
}

#[test]
fn a_concrete_advertised_id_carries_its_price() {
    let payload = models_payload();
    let fable = payload["providers"][0]["models"][3].clone();
    assert_eq!(fable["id"], "claude-fable-5-1[1m]");
    assert_eq!(fable["cost_per_million_input"], 10.0);
    assert_eq!(fable["cost_per_million_output"], 50.0);
}

#[test]
fn bare_aliases_report_no_price_rather_than_a_guessed_one() {
    let payload = models_payload();
    for i in [0usize, 1, 2] {
        let model = payload["providers"][0]["models"][i].clone();
        assert!(
            model["cost_per_million_input"].is_null(),
            "alias {} must not be priced",
            model["id"]
        );
    }
}

#[test]
fn pricing_reference_is_reported_separately_from_availability() {
    // The registry still ships, but as a pricing table under its own key —
    // never conflated with what is installed.
    let payload = models_payload();
    let reference = payload["pricing_reference"].as_array().unwrap();
    assert!(reference.iter().any(|e| e["id"] == "claude-opus-5"));
    // ...and Opus 5 is NOT claimed as an available model.
    let available: Vec<&str> = payload["providers"][0]["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert!(!available.contains(&"claude-opus-5"));
}

#[test]
fn an_unprobed_provider_is_reported_as_unavailable() {
    use crate::agent::probe::ProviderModelSet;
    let registry = crate::models::ModelRegistry::defaults();
    let payload = super::cmd_models::build_models_json(
        &registry,
        &[ProviderModelSet {
            provider_id: "copilot".into(),
            label: "GitHub Copilot".into(),
            models: vec![],
            probed: false,
        }],
    );
    assert_eq!(payload["providers"][0]["probed"], false);
    assert!(
        payload["providers"][0]["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn json_includes_backends() {
    let payload = models_payload();
    let backends = payload["backends"].as_array().unwrap();
    assert!(!backends.is_empty());
    assert!(backends.iter().all(|b| b["name"].is_string()));
}

#[test]
fn backend_capabilities_included_in_all_compiled() {
    // Every compiled backend should return valid capabilities.
    for backend in Backend::all_compiled() {
        let caps = backend.capabilities();
        assert!(!caps.name.is_empty());
        assert_eq!(caps.feature_list().len(), 8);
    }
}
