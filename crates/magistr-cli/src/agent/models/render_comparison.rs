//! Render a comparison result as human-readable plain text.

#![allow(dead_code)]

use crate::utils::text;

use super::comparison_result::ComparisonResult;

/// Render a comparison result as a human-readable plain text report.
///
/// Used for `--no-tui` mode and as a building block for TUI rendering.
#[must_use]
pub fn render_comparison(result: &ComparisonResult) -> String {
    let mut out = String::new();

    // Header
    out.push_str("━━━ Model Comparison ━━━\n\n");
    out.push_str(&format!(
        "Prompt: \"{}\"\n\n",
        text::truncate(&result.prompt, 80)
    ));

    // Model A
    out.push_str(&format!(
        "┌─ {} ─────────────────────────────\n",
        result.response_a.display_name
    ));
    out.push_str(&format!("│ Cost:     ${:.4}\n", result.response_a.cost_usd));
    out.push_str(&format!(
        "│ Duration: {}ms\n",
        result.response_a.duration_ms
    ));
    out.push_str(&format!(
        "│ Tokens:   {} in / {} out\n",
        result.response_a.tokens.input_tokens, result.response_a.tokens.output_tokens
    ));
    out.push_str(&format!(
        "│ Speed:    {:.1} tok/s\n",
        result.response_a.tokens_per_second()
    ));
    out.push_str("│\n");
    for line in result.response_a.output.lines().take(20) {
        out.push_str(&format!("│ {line}\n"));
    }
    if result.response_a.output.lines().count() > 20 {
        out.push_str("│ ... (truncated)\n");
    }
    out.push_str("└──────────────────────────────────\n\n");

    // Model B
    out.push_str(&format!(
        "┌─ {} ─────────────────────────────\n",
        result.response_b.display_name
    ));
    out.push_str(&format!("│ Cost:     ${:.4}\n", result.response_b.cost_usd));
    out.push_str(&format!(
        "│ Duration: {}ms\n",
        result.response_b.duration_ms
    ));
    out.push_str(&format!(
        "│ Tokens:   {} in / {} out\n",
        result.response_b.tokens.input_tokens, result.response_b.tokens.output_tokens
    ));
    out.push_str(&format!(
        "│ Speed:    {:.1} tok/s\n",
        result.response_b.tokens_per_second()
    ));
    out.push_str("│\n");
    for line in result.response_b.output.lines().take(20) {
        out.push_str(&format!("│ {line}\n"));
    }
    if result.response_b.output.lines().count() > 20 {
        out.push_str("│ ... (truncated)\n");
    }
    out.push_str("└──────────────────────────────────\n\n");

    // Summary
    out.push_str("━━━ Summary ━━━\n\n");
    out.push_str(&format!(
        "  Cheaper:  {} (saves {:.1}%)\n",
        result.cheaper_model(),
        result.cost_savings_percent()
    ));
    out.push_str(&format!(
        "  Faster:   {} ({:.1}% faster)\n",
        result.faster_model(),
        result.speed_difference_percent()
    ));
    out.push_str(&format!(
        "  Cost Δ:   ${:.4}\n",
        result.cost_difference().abs()
    ));

    out
}
