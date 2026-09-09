//! Built-in Claude model definitions.

use super::model_config::ModelConfig;
use super::provider::Provider;

/// Built-in Claude model definitions.
///
/// Always available regardless of config file. User config can override
/// these by providing a model with the same `id`.
///
/// Ordered most to least capable — [`crate::models::ModelRegistry::defaults`]
/// preserves this order, and prices are per million tokens.
///
/// Claude Fable 5 is listed here so `magistr models`, `magistr compare` and
/// `--model` can reach it, but it is deliberately absent from the automatic
/// routing chains in [`crate::models::route`]: at $10/$50 it is the priciest
/// model in the lineup, so escalating onto it stays an explicit choice.
pub fn builtin_models() -> Vec<ModelConfig> {
    vec![
        ModelConfig {
            id: "claude-fable-5".into(),
            provider: Provider::Claude,
            display_name: "Claude Fable 5".into(),
            cost_per_million_input: 10.0,
            cost_per_million_output: 50.0,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            base_url: None,
        },
        ModelConfig {
            id: "claude-opus-5".into(),
            provider: Provider::Claude,
            display_name: "Claude Opus 5".into(),
            cost_per_million_input: 5.0,
            cost_per_million_output: 25.0,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            base_url: None,
        },
        ModelConfig {
            id: "claude-sonnet-5".into(),
            provider: Provider::Claude,
            display_name: "Claude Sonnet 5".into(),
            cost_per_million_input: 2.0,
            cost_per_million_output: 10.0,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            base_url: None,
        },
        ModelConfig {
            id: "claude-haiku-4-5".into(),
            provider: Provider::Claude,
            display_name: "Claude Haiku 4.5".into(),
            cost_per_million_input: 1.0,
            cost_per_million_output: 5.0,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            base_url: None,
        },
    ]
}
