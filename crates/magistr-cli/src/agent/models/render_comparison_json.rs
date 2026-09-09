//! Render a comparison result as JSON.

#![allow(dead_code)]

use super::comparison_result::ComparisonResult;

/// Render a comparison result as a JSON string for machine consumption.
pub fn render_comparison_json(result: &ComparisonResult) -> Result<String, serde_json::Error> {
    let value = serde_json::json!({
        "prompt": result.prompt,
        "model_a": {
            "model_id": result.response_a.model_id,
            "display_name": result.response_a.display_name,
            "cost_usd": result.response_a.cost_usd,
            "duration_ms": result.response_a.duration_ms,
            "tokens": {
                "input": result.response_a.tokens.input_tokens,
                "output": result.response_a.tokens.output_tokens,
                "cache_read": result.response_a.tokens.cache_read_tokens,
                "cache_write": result.response_a.tokens.cache_write_tokens,
            },
            "output_length": result.response_a.output_len(),
            "tokens_per_second": result.response_a.tokens_per_second(),
        },
        "model_b": {
            "model_id": result.response_b.model_id,
            "display_name": result.response_b.display_name,
            "cost_usd": result.response_b.cost_usd,
            "duration_ms": result.response_b.duration_ms,
            "tokens": {
                "input": result.response_b.tokens.input_tokens,
                "output": result.response_b.tokens.output_tokens,
                "cache_read": result.response_b.tokens.cache_read_tokens,
                "cache_write": result.response_b.tokens.cache_write_tokens,
            },
            "output_length": result.response_b.output_len(),
            "tokens_per_second": result.response_b.tokens_per_second(),
        },
        "summary": {
            "cheaper_model": result.cheaper_model(),
            "faster_model": result.faster_model(),
            "cost_difference_usd": result.cost_difference().abs(),
            "cost_savings_percent": result.cost_savings_percent(),
            "speed_difference_percent": result.speed_difference_percent(),
        },
    });
    serde_json::to_string_pretty(&value)
}
