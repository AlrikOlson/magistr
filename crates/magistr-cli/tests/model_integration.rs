//! Integration tests for model configuration, registry, and comparison.
//!
//! Tests cross-module interactions between `models::config`, `models::registry`,
//! `models::compare`, and `cost::types`.

use tempfile::TempDir;

use magistr::cost::types::TokenBreakdown;
use magistr::models::compare::{
    ComparisonRequest, ComparisonResult, ModelResponse, render_comparison, render_comparison_json,
};
use magistr::models::config::{
    ModelConfig, ModelsFile, Provider, builtin_models, load_from, save_to,
};

// ── Helpers ────────────────────────────────────────────────────────

fn sample_model(id: &str, provider: Provider, input_cost: f64, output_cost: f64) -> ModelConfig {
    ModelConfig {
        id: id.into(),
        provider,
        display_name: format!("Test {id}"),
        cost_per_million_input: input_cost,
        cost_per_million_output: output_cost,
        api_key_env: None,
        base_url: None,
    }
}

fn sample_response(model_id: &str, name: &str, cost: f64, duration_ms: u64) -> ModelResponse {
    ModelResponse {
        model_id: model_id.into(),
        display_name: name.into(),
        output: format!("Response from {name}"),
        tokens: TokenBreakdown {
            input_tokens: 500,
            output_tokens: 200,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
        cost_usd: cost,
        duration_ms,
    }
}

// ── Config save/load with builtins ─────────────────────────────────

#[test]
fn config_save_load_round_trip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("models.toml");

    let file = ModelsFile {
        models: vec![
            sample_model("custom-model", Provider::Local, 0.0, 0.0),
            sample_model("gpt-4o", Provider::OpenAi, 5.0, 15.0),
        ],
    };

    save_to(&file, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.models.len(), 2);
    assert_eq!(loaded.models[0].id, "custom-model");
    assert_eq!(loaded.models[0].provider, Provider::Local);
    assert_eq!(loaded.models[1].id, "gpt-4o");
    assert_eq!(loaded.models[1].provider, Provider::OpenAi);
}

// ── Builtin models are always available ────────────────────────────

#[test]
fn builtin_models_available() {
    let builtins = builtin_models();
    assert!(builtins.len() >= 3); // At least 3 Claude models

    // All are Claude provider
    for m in &builtins {
        assert_eq!(m.provider, Provider::Claude);
    }

    // IDs are unique
    let ids: Vec<&str> = builtins.iter().map(|m| m.id.as_str()).collect();
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len());
}

// ── Model cost calculation ─────────────────────────────────────────

#[test]
fn cost_calculation_across_models() {
    let cheap = sample_model("cheap", Provider::Local, 1.0, 2.0);
    let expensive = sample_model("expensive", Provider::Claude, 15.0, 75.0);

    // 1M input + 1M output
    let cheap_cost = cheap.calculate_cost(1_000_000, 1_000_000);
    let expensive_cost = expensive.calculate_cost(1_000_000, 1_000_000);

    assert!((cheap_cost - 3.0).abs() < f64::EPSILON); // 1.0 + 2.0
    assert!((expensive_cost - 90.0).abs() < f64::EPSILON); // 15.0 + 75.0
    assert!(expensive_cost > cheap_cost);
}

// ── Comparison request validation ──────────────────────────────────

#[test]
fn comparison_request_validation() {
    let valid = ComparisonRequest::new("test prompt", "model-a", "model-b");
    assert!(valid.is_valid());

    let same_model = ComparisonRequest::new("test", "model-a", "model-a");
    assert!(!same_model.is_valid());

    let empty_prompt = ComparisonRequest::new("", "model-a", "model-b");
    assert!(!empty_prompt.is_valid());
}

// ── Comparison result analysis ─────────────────────────────────────

#[test]
fn comparison_result_analysis() {
    let resp_a = sample_response("opus", "Claude Opus", 0.50, 5000);
    let resp_b = sample_response("sonnet", "Claude Sonnet", 0.15, 2000);

    let result = ComparisonResult::new("Compare these models", resp_a, resp_b);

    // Cost analysis
    assert!((result.cost_difference() - 0.35).abs() < f64::EPSILON);
    assert_eq!(result.cheaper_model(), "Claude Sonnet");

    // Speed analysis
    assert_eq!(result.faster_model(), "Claude Sonnet");

    // Savings
    let savings = result.cost_savings_percent();
    assert!(savings > 0.0);
    assert!(savings < 100.0);
}

// ── Comparison rendering ───────────────────────────────────────────

#[test]
fn comparison_render_contains_key_info() {
    let resp_a = sample_response("opus", "Claude Opus", 0.50, 5000);
    let resp_b = sample_response("sonnet", "Claude Sonnet", 0.15, 2000);
    let result = ComparisonResult::new("Test prompt", resp_a, resp_b);

    let rendered = render_comparison(&result);
    assert!(rendered.contains("Claude Opus"));
    assert!(rendered.contains("Claude Sonnet"));
    assert!(rendered.contains("Test prompt"));
    // Contains cost info
    assert!(rendered.contains("$0.50") || rendered.contains("0.50"));
}

// ── JSON comparison output ─────────────────────────────────────────

#[test]
fn comparison_json_round_trip() {
    let resp_a = sample_response("opus", "Claude Opus", 0.50, 5000);
    let resp_b = sample_response("sonnet", "Claude Sonnet", 0.15, 2000);
    let result = ComparisonResult::new("Test prompt", resp_a, resp_b);

    let json_str = render_comparison_json(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Verify structure
    assert!(parsed.get("prompt").is_some());
    assert!(parsed.get("model_a").is_some());
    assert!(parsed.get("model_b").is_some());
    assert!(parsed.get("summary").is_some());

    // Verify summary fields
    let summary = &parsed["summary"];
    assert!(summary.get("cheaper_model").is_some());
    assert!(summary.get("faster_model").is_some());
    assert!(summary.get("cost_savings_percent").is_some());
}

// ── Missing config file returns defaults ───────────────────────────

#[test]
fn missing_config_returns_empty_models() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nonexistent.toml");

    let file = load_from(&path).unwrap();
    assert!(file.models.is_empty());
}

// ── Invalid TOML produces error ────────────────────────────────────

#[test]
fn invalid_toml_returns_error() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("bad.toml");
    std::fs::write(&path, "this is not valid toml [[[").unwrap();

    let result = load_from(&path);
    assert!(result.is_err());
}
