use super::*;
use crate::cost::types::TokenBreakdown;
use crate::utils::text;

fn sample_tokens() -> TokenBreakdown {
    TokenBreakdown {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_write_tokens: 100,
    }
}

fn response_a() -> ModelResponse {
    ModelResponse {
        model_id: "claude-opus-4".to_string(),
        display_name: "Claude Opus 4".to_string(),
        output: "This is the Opus response.\nIt has multiple lines.".to_string(),
        tokens: TokenBreakdown {
            input_tokens: 1000,
            output_tokens: 800,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
        cost_usd: 0.045,
        duration_ms: 5000,
    }
}

fn response_b() -> ModelResponse {
    ModelResponse {
        model_id: "claude-sonnet-4".to_string(),
        display_name: "Claude Sonnet 4".to_string(),
        output: "This is the Sonnet response.\nShorter but fast.".to_string(),
        tokens: TokenBreakdown {
            input_tokens: 1000,
            output_tokens: 400,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
        cost_usd: 0.012,
        duration_ms: 2000,
    }
}

// ── ComparisonRequest ──────────────────────────────────────

#[test]
fn request_new() {
    let req = ComparisonRequest::new("test prompt", "opus", "sonnet");
    assert_eq!(req.prompt, "test prompt");
    assert_eq!(req.model_a, "opus");
    assert_eq!(req.model_b, "sonnet");
}

#[test]
fn request_valid_when_different_models() {
    let req = ComparisonRequest::new("test", "opus", "sonnet");
    assert!(req.is_valid());
}

#[test]
fn request_invalid_when_same_models() {
    let req = ComparisonRequest::new("test", "opus", "opus");
    assert!(!req.is_valid());
}

#[test]
fn request_invalid_when_empty_prompt() {
    let req = ComparisonRequest::new("", "opus", "sonnet");
    assert!(!req.is_valid());
}

// ── ModelResponse ──────────────────────────────────────────

#[test]
fn tokens_per_second_calculation() {
    let resp = response_a();
    // 800 output tokens / (5000ms / 1000) = 800/5 = 160 tok/s
    assert!((resp.tokens_per_second() - 160.0).abs() < 0.01);
}

#[test]
fn tokens_per_second_zero_duration() {
    let resp = ModelResponse {
        duration_ms: 0,
        ..response_a()
    };
    assert_eq!(resp.tokens_per_second(), 0.0);
}

#[test]
fn output_len() {
    let resp = response_a();
    assert_eq!(resp.output_len(), resp.output.len());
}

// ── ComparisonResult ───────────────────────────────────────

#[test]
fn cost_difference_positive_when_a_more_expensive() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    // 0.045 - 0.012 = 0.033
    assert!((result.cost_difference() - 0.033).abs() < 0.0001);
}

#[test]
fn cost_difference_negative_when_b_more_expensive() {
    let result = ComparisonResult::new("test", response_b(), response_a());
    assert!(result.cost_difference() < 0.0);
}

#[test]
fn cheaper_model_is_sonnet() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    assert_eq!(result.cheaper_model(), "Claude Sonnet 4");
}

#[test]
fn cheaper_model_when_a_cheaper() {
    let result = ComparisonResult::new("test", response_b(), response_a());
    assert_eq!(result.cheaper_model(), "Claude Sonnet 4");
}

#[test]
fn faster_model_is_sonnet() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    assert_eq!(result.faster_model(), "Claude Sonnet 4");
}

#[test]
fn cost_savings_percent_calculation() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    // max = 0.045, min = 0.012, savings = (0.045-0.012)/0.045 * 100 = 73.33%
    assert!((result.cost_savings_percent() - 73.33).abs() < 0.1);
}

#[test]
fn cost_savings_zero_when_equal() {
    let a = response_a();
    let b = ModelResponse {
        cost_usd: a.cost_usd,
        ..response_b()
    };
    let result = ComparisonResult::new("test", a, b);
    assert_eq!(result.cost_savings_percent(), 0.0);
}

#[test]
fn cost_savings_zero_when_both_free() {
    let a = ModelResponse {
        cost_usd: 0.0,
        ..response_a()
    };
    let b = ModelResponse {
        cost_usd: 0.0,
        ..response_b()
    };
    let result = ComparisonResult::new("test", a, b);
    assert_eq!(result.cost_savings_percent(), 0.0);
}

#[test]
fn speed_difference_percent_calculation() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    // max = 5000, min = 2000, diff = (5000-2000)/5000 * 100 = 60%
    assert!((result.speed_difference_percent() - 60.0).abs() < 0.1);
}

#[test]
fn speed_difference_zero_when_equal() {
    let a = response_a();
    let b = ModelResponse {
        duration_ms: a.duration_ms,
        ..response_b()
    };
    let result = ComparisonResult::new("test", a, b);
    assert_eq!(result.speed_difference_percent(), 0.0);
}

// ── render_comparison ──────────────────────────────────────

#[test]
fn render_contains_model_names() {
    let result = ComparisonResult::new("test prompt", response_a(), response_b());
    let rendered = render_comparison(&result);
    assert!(
        rendered.contains("Claude Opus 4"),
        "should contain model A name"
    );
    assert!(
        rendered.contains("Claude Sonnet 4"),
        "should contain model B name"
    );
}

#[test]
fn render_contains_costs() {
    let result = ComparisonResult::new("test prompt", response_a(), response_b());
    let rendered = render_comparison(&result);
    assert!(rendered.contains("$0.0450"), "should contain model A cost");
    assert!(rendered.contains("$0.0120"), "should contain model B cost");
}

#[test]
fn render_contains_summary() {
    let result = ComparisonResult::new("test prompt", response_a(), response_b());
    let rendered = render_comparison(&result);
    assert!(rendered.contains("Cheaper:"), "should have cheaper label");
    assert!(rendered.contains("Faster:"), "should have faster label");
    assert!(rendered.contains("Cost Δ:"), "should have cost delta");
}

#[test]
fn render_contains_output() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    let rendered = render_comparison(&result);
    assert!(
        rendered.contains("Opus response"),
        "should contain model A output"
    );
    assert!(
        rendered.contains("Sonnet response"),
        "should contain model B output"
    );
}

#[test]
fn render_contains_prompt() {
    let result = ComparisonResult::new("explain functions", response_a(), response_b());
    let rendered = render_comparison(&result);
    assert!(rendered.contains("explain functions"));
}

#[test]
fn render_truncates_long_output() {
    let long_output: String = (0..30)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let a = ModelResponse {
        output: long_output,
        ..response_a()
    };
    let result = ComparisonResult::new("test", a, response_b());
    let rendered = render_comparison(&result);
    assert!(rendered.contains("... (truncated)"));
}

// ── render_comparison_json ─────────────────────────────────

#[test]
fn render_json_is_valid() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    let json = render_comparison_json(&result).expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
    assert!(parsed["model_a"]["cost_usd"].is_number());
    assert!(parsed["model_b"]["cost_usd"].is_number());
    assert!(parsed["summary"]["cheaper_model"].is_string());
}

#[test]
fn render_json_contains_token_breakdown() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    let json = render_comparison_json(&result).expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
    assert_eq!(parsed["model_a"]["tokens"]["input"], 1000);
    assert_eq!(parsed["model_a"]["tokens"]["output"], 800);
}

#[test]
fn render_json_summary_fields() {
    let result = ComparisonResult::new("test", response_a(), response_b());
    let json = render_comparison_json(&result).expect("should serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse");
    assert_eq!(parsed["summary"]["cheaper_model"], "Claude Sonnet 4");
    assert_eq!(parsed["summary"]["faster_model"], "Claude Sonnet 4");
}

// ── truncate (delegates to utils::text) ─────────────────────

#[test]
fn truncate_short_string_unchanged() {
    assert_eq!(text::truncate("hello", 10), "hello");
}

#[test]
fn truncate_exact_length_unchanged() {
    assert_eq!(text::truncate("hello", 5), "hello");
}

#[test]
fn truncate_long_string_adds_ellipsis() {
    assert_eq!(text::truncate("hello world", 6), "hello…");
}

#[test]
fn truncate_minimum_length() {
    assert_eq!(text::truncate("hello", 1), "…");
}
