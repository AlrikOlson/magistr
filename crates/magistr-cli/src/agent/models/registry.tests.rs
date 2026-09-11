use crate::in_env as env_test;

use super::*;
use crate::agent::models::config::{ModelConfig, ModelsFile, Provider, builtin_models};

fn openai_model() -> ModelConfig {
    ModelConfig {
        id: "gpt-4o".into(),
        provider: Provider::OpenAi,
        display_name: "GPT-4o".into(),
        cost_per_million_input: 2.5,
        cost_per_million_output: 10.0,
        api_key_env: Some("MAGISTR_TEST_NONEXISTENT_OPENAI_KEY".into()),
        base_url: None,
    }
}

fn local_model() -> ModelConfig {
    ModelConfig {
        id: "llama3".into(),
        provider: Provider::Local,
        display_name: "Llama 3 70B".into(),
        cost_per_million_input: 0.0,
        cost_per_million_output: 0.0,
        api_key_env: None,
        base_url: Some("http://localhost:11434".into()),
    }
}

// ── defaults ───────────────────────────────────────────────

#[test]
fn defaults_has_every_builtin_model() {
    let reg = ModelRegistry::defaults();
    assert_eq!(reg.len(), builtin_models().len());
    assert!(!reg.is_empty());
}

#[test]
fn defaults_all_are_builtin_source() {
    let reg = ModelRegistry::defaults();
    for entry in reg.all() {
        assert_eq!(entry.source, ModelSource::Builtin);
    }
}

#[test]
fn defaults_can_find_opus() {
    let reg = ModelRegistry::defaults();
    let opus = reg.get("claude-opus-5");
    assert!(opus.is_some());
    assert_eq!(opus.unwrap().config.display_name, "Claude Opus 5");
}

#[test]
fn defaults_can_find_fable() {
    // Fable 5 is registry-only — never auto-routed, but reachable by id.
    let reg = ModelRegistry::defaults();
    let fable = reg.get("claude-fable-5-1");
    assert!(fable.is_some());
    assert_eq!(fable.unwrap().config.display_name, "Claude Fable 5.1");
}

// ── from_config ────────────────────────────────────────────

#[test]
fn from_config_adds_new_models() {
    let user = ModelsFile {
        models: vec![openai_model(), local_model()],
    };
    let reg = ModelRegistry::from_config(&user);
    assert_eq!(reg.len(), builtin_models().len() + 2);
}

#[test]
fn from_config_overrides_builtin_by_id() {
    let mut custom = builtin_models()[0].clone();
    let overridden_id = custom.id.clone();
    custom.display_name = "My Custom Model".into();
    custom.cost_per_million_input = 1.0; // cheaper!

    let user = ModelsFile {
        models: vec![custom],
    };
    let reg = ModelRegistry::from_config(&user);

    assert_eq!(reg.len(), builtin_models().len()); // override, not add
    let entry = reg.get(&overridden_id).unwrap();
    assert_eq!(entry.config.display_name, "My Custom Model");
    assert_eq!(entry.source, ModelSource::UserConfig);
}

#[test]
fn from_config_empty_file_matches_defaults() {
    let reg = ModelRegistry::from_config(&ModelsFile::default());
    assert_eq!(reg.len(), ModelRegistry::defaults().len());
}

// ── get ────────────────────────────────────────────────────

#[test]
fn get_nonexistent_returns_none() {
    let reg = ModelRegistry::defaults();
    assert!(reg.get("nonexistent-model").is_none());
}

// ── by_provider ────────────────────────────────────────────

#[test]
fn by_provider_filters_correctly() {
    let user = ModelsFile {
        models: vec![openai_model(), local_model()],
    };
    let reg = ModelRegistry::from_config(&user);

    assert_eq!(
        reg.by_provider(Provider::Claude).len(),
        builtin_models().len()
    );
    assert_eq!(reg.by_provider(Provider::OpenAi).len(), 1);
    assert_eq!(reg.by_provider(Provider::Local).len(), 1);
}

// ── available/unavailable ──────────────────────────────────

#[test]
fn local_model_always_available() {
    let user = ModelsFile {
        models: vec![local_model()],
    };
    let reg = ModelRegistry::from_config(&user);
    let available = reg.available();
    assert!(available.iter().any(|e| e.config.id == "llama3"));
}

#[test]
fn model_with_missing_key_is_unavailable() {
    if !env_test::in_env(&[("MAGISTR_TEST_NONEXISTENT_OPENAI_KEY", None)]) {
        return;
    }
    let user = ModelsFile {
        models: vec![openai_model()],
    };
    let reg = ModelRegistry::from_config(&user);
    let unavailable = reg.unavailable();
    assert!(unavailable.iter().any(|e| e.config.id == "gpt-4o"));
}

// ── ModelSource ────────────────────────────────────────────

#[test]
fn model_source_labels() {
    assert_eq!(ModelSource::Builtin.label(), "built-in");
    assert_eq!(ModelSource::UserConfig.label(), "user config");
}

#[test]
fn model_source_display() {
    assert_eq!(format!("{}", ModelSource::Builtin), "built-in");
    assert_eq!(format!("{}", ModelSource::UserConfig), "user config");
}
