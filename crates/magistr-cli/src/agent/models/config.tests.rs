use crate::in_env as env_test;

use std::path::PathBuf;

use super::*;

// ── Provider ───────────────────────────────────────────────

#[test]
fn provider_labels() {
    assert_eq!(Provider::Claude.label(), "Claude");
    assert_eq!(Provider::OpenAi.label(), "OpenAI");
    assert_eq!(Provider::Local.label(), "Local");
}

#[test]
fn provider_display() {
    assert_eq!(format!("{}", Provider::Claude), "Claude");
    assert_eq!(format!("{}", Provider::OpenAi), "OpenAI");
}

// ── ModelConfig ────────────────────────────────────────────

#[test]
fn model_available_when_no_key_required() {
    let model = ModelConfig {
        id: "llama3".into(),
        provider: Provider::Local,
        display_name: "Llama 3".into(),
        cost_per_million_input: 0.0,
        cost_per_million_output: 0.0,
        api_key_env: None,
        base_url: Some("http://localhost:11434".into()),
    };
    assert!(model.is_available());
}

#[test]
fn model_unavailable_when_key_missing() {
    if !env_test::in_env(&[("MAGISTR_TEST_NONEXISTENT_KEY_12345", None)]) {
        return;
    }
    let model = ModelConfig {
        id: "test-model".into(),
        provider: Provider::OpenAi,
        display_name: "Test".into(),
        cost_per_million_input: 5.0,
        cost_per_million_output: 15.0,
        api_key_env: Some("MAGISTR_TEST_NONEXISTENT_KEY_12345".into()),
        base_url: None,
    };
    assert!(!model.is_available());
}

#[test]
fn calculate_cost_zero_tokens() {
    let model = &builtin_models()[0]; // fable 5
    assert_eq!(model.calculate_cost(0, 0), 0.0);
}

#[test]
fn calculate_cost_one_million_tokens() {
    let model = &builtin_models()[0]; // fable 5: $10/M in, $50/M out
    let cost = model.calculate_cost(1_000_000, 1_000_000);
    assert!((cost - 60.0).abs() < f64::EPSILON);
}

#[test]
fn calculate_cost_realistic_usage() {
    let model = &builtin_models()[1]; // opus 5: $5/M in, $25/M out
    // 10K input, 2K output
    let cost = model.calculate_cost(10_000, 2_000);
    let expected = (10_000.0 / 1_000_000.0) * 5.0 + (2_000.0 / 1_000_000.0) * 25.0;
    assert!((cost - expected).abs() < 1e-10);
}

// ── Built-in models ────────────────────────────────────────

#[test]
fn builtin_models_are_the_current_lineup() {
    let models = builtin_models();
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "claude-fable-5-1",
            "claude-opus-5",
            "claude-sonnet-5",
            "claude-haiku-4-5",
        ]
    );
}

#[test]
fn builtin_models_all_claude_provider() {
    for model in &builtin_models() {
        assert_eq!(model.provider, Provider::Claude);
    }
}

#[test]
fn builtin_models_all_need_anthropic_key() {
    for model in &builtin_models() {
        assert_eq!(model.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
    }
}

#[test]
fn builtin_models_ids_are_unique() {
    let models = builtin_models();
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    let mut deduped = ids.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(ids.len(), deduped.len());
}

#[test]
fn builtins_are_ordered_most_to_least_expensive() {
    // Routing and `magistr compare` both read cost as the capability proxy,
    // so the declared order has to match the price order.
    let models = builtin_models();
    for pair in models.windows(2) {
        assert!(
            pair[0].cost_per_million_input >= pair[1].cost_per_million_input,
            "{} should not be cheaper than {}",
            pair[0].id,
            pair[1].id
        );
        assert!(
            pair[0].cost_per_million_output >= pair[1].cost_per_million_output,
            "{} should not be cheaper than {}",
            pair[0].id,
            pair[1].id
        );
    }
}

// ── File I/O ───────────────────────────────────────────────

#[test]
fn load_missing_file_returns_default() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("models.toml");
    let file = load_from(&path).unwrap();
    assert_eq!(file, ModelsFile::default());
}

#[test]
fn save_and_load_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("models.toml");

    let file = ModelsFile {
        models: vec![ModelConfig {
            id: "gpt-4o".into(),
            provider: Provider::OpenAi,
            display_name: "GPT-4o".into(),
            cost_per_million_input: 2.5,
            cost_per_million_output: 10.0,
            api_key_env: Some("OPENAI_API_KEY".into()),
            base_url: None,
        }],
    };

    save_to(&file, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded, file);
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nested").join("models.toml");

    save_to(&ModelsFile::default(), &path).unwrap();
    assert!(path.is_file());
}

#[test]
fn load_invalid_toml_returns_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("models.toml");
    std::fs::write(&path, "not valid [[[ toml").unwrap();

    let result = load_from(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("parse"));
}

#[test]
fn load_empty_file_returns_default() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("models.toml");
    std::fs::write(&path, "").unwrap();

    let file = load_from(&path).unwrap();
    assert_eq!(file, ModelsFile::default());
}

#[test]
fn load_toml_with_local_model() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("models.toml");
    std::fs::write(
        &path,
        r#"
[[models]]
id = "llama3"
provider = "local"
display_name = "Llama 3 70B"
cost_per_million_input = 0.0
cost_per_million_output = 0.0
base_url = "http://localhost:11434"
"#,
    )
    .unwrap();

    let file = load_from(&path).unwrap();
    assert_eq!(file.models.len(), 1);
    assert_eq!(file.models[0].id, "llama3");
    assert_eq!(file.models[0].provider, Provider::Local);
    assert!(file.models[0].api_key_env.is_none());
    assert_eq!(
        file.models[0].base_url.as_deref(),
        Some("http://localhost:11434")
    );
}

#[test]
fn load_toml_with_openai_alias() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("models.toml");
    std::fs::write(
        &path,
        r#"
[[models]]
id = "gpt-4o"
provider = "openai"
display_name = "GPT-4o"
cost_per_million_input = 2.5
cost_per_million_output = 10.0
api_key_env = "OPENAI_API_KEY"
"#,
    )
    .unwrap();

    let file = load_from(&path).unwrap();
    assert_eq!(file.models[0].provider, Provider::OpenAi);
}

// ── Error display ──────────────────────────────────────────

#[test]
fn error_display_messages() {
    let io_err = ModelConfigError::Io(
        PathBuf::from("/tmp/test"),
        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    );
    assert!(io_err.to_string().contains("model config I/O error"));

    let parse_err = ModelConfigError::Parse(PathBuf::from("/tmp/test"), "bad syntax".into());
    assert!(parse_err.to_string().contains("parse models config"));

    let ser_err = ModelConfigError::Serialize("encoding failed".into());
    assert!(ser_err.to_string().contains("serialize models config"));
}
