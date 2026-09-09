use super::*;

// --- ContextCategory tests ---

#[test]
fn category_all_has_four() {
    assert_eq!(ContextCategory::ALL.len(), 4);
}

#[test]
fn category_display() {
    assert_eq!(ContextCategory::SystemPrompt.to_string(), "System Prompt");
    assert_eq!(
        ContextCategory::ConversationHistory.to_string(),
        "Conversation"
    );
    assert_eq!(ContextCategory::FileContext.to_string(), "File Context");
    assert_eq!(ContextCategory::ToolResults.to_string(), "Tool Results");
}

// --- WarningSeverity tests ---

#[test]
fn severity_ordering() {
    assert!(WarningSeverity::Info < WarningSeverity::Warning);
    assert!(WarningSeverity::Warning < WarningSeverity::Critical);
}

#[test]
fn severity_display() {
    assert_eq!(WarningSeverity::Info.to_string(), "info");
    assert_eq!(WarningSeverity::Warning.to_string(), "warning");
    assert_eq!(WarningSeverity::Critical.to_string(), "critical");
}

// --- format_tokens tests ---

#[test]
fn format_tokens_small() {
    assert_eq!(format_tokens(0), "0 tokens");
    assert_eq!(format_tokens(500), "500 tokens");
    assert_eq!(format_tokens(999), "999 tokens");
}

#[test]
fn format_tokens_thousands() {
    assert_eq!(format_tokens(1_000), "1.0K tokens");
    assert_eq!(format_tokens(1_500), "1.5K tokens");
    assert_eq!(format_tokens(10_000), "10K tokens");
    assert_eq!(format_tokens(38_000), "38K tokens");
    assert_eq!(format_tokens(99_999), "100K tokens");
}

#[test]
fn format_tokens_millions() {
    assert_eq!(format_tokens(1_000_000), "1.0M tokens");
    assert_eq!(format_tokens(1_500_000), "1.5M tokens");
    assert_eq!(format_tokens(10_000_000), "10M tokens");
}

// --- ContextHealth basic tests ---

#[test]
fn new_health_starts_at_zero() {
    let health = ContextHealth::new(200_000);
    assert_eq!(health.total_tokens(), 0);
    assert_eq!(health.context_window(), 200_000);
    assert!((health.usage_fraction()).abs() < f64::EPSILON);
    assert!((health.usage_percent()).abs() < f64::EPSILON);
}

#[test]
fn default_uses_200k_window() {
    let health = ContextHealth::default();
    assert_eq!(health.context_window(), DEFAULT_CONTEXT_WINDOW);
}

#[test]
fn zero_window_returns_zero_fraction() {
    let health = ContextHealth::new(0);
    assert!((health.usage_fraction()).abs() < f64::EPSILON);
}

#[test]
fn total_tokens_sums_all_categories() {
    let mut health = ContextHealth::new(200_000);
    health.set_system_prompt_tokens(5_000);
    health.set_conversation_tokens(20_000);
    health.set_file_context_tokens(10_000);
    health.set_tool_results_tokens(3_000);
    assert_eq!(health.total_tokens(), 38_000);
}

#[test]
fn usage_fraction_calculation() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(42_000);
    let fraction = health.usage_fraction();
    assert!((fraction - 0.42).abs() < f64::EPSILON);
}

#[test]
fn usage_percent_calculation() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(42_000);
    assert!((health.usage_percent() - 42.0).abs() < f64::EPSILON);
}

#[test]
fn usage_can_exceed_100_percent() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(120_000);
    assert!(health.usage_percent() > 100.0);
}

// --- usage_display tests ---

#[test]
fn usage_display_format() {
    let mut health = ContextHealth::new(200_000);
    health.set_system_prompt_tokens(5_000);
    health.set_conversation_tokens(33_000);
    let display = health.usage_display();
    assert!(display.contains("19%"), "got: {display}");
    assert!(display.contains("38K tokens"), "got: {display}");
    assert!(display.contains("200K tokens"), "got: {display}");
}

// --- breakdown tests ---

#[test]
fn breakdown_returns_all_categories() {
    let mut health = ContextHealth::new(200_000);
    health.set_system_prompt_tokens(5_000);
    health.set_conversation_tokens(20_000);
    health.set_file_context_tokens(10_000);
    health.set_tool_results_tokens(3_000);

    let breakdown = health.breakdown();
    assert_eq!(breakdown.len(), 4);
    assert_eq!(breakdown[0].category, ContextCategory::SystemPrompt);
    assert_eq!(breakdown[0].tokens, 5_000);
    assert_eq!(breakdown[1].category, ContextCategory::ConversationHistory);
    assert_eq!(breakdown[1].tokens, 20_000);
    assert_eq!(breakdown[2].category, ContextCategory::FileContext);
    assert_eq!(breakdown[2].tokens, 10_000);
    assert_eq!(breakdown[3].category, ContextCategory::ToolResults);
    assert_eq!(breakdown[3].tokens, 3_000);
}

#[test]
fn breakdown_zero_when_empty() {
    let health = ContextHealth::new(200_000);
    let breakdown = health.breakdown();
    for usage in &breakdown {
        assert_eq!(usage.tokens, 0);
    }
}

// --- warning tests ---

#[test]
fn no_warning_when_under_70_percent() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(60_000);
    assert!(health.check_warning().is_none());
}

#[test]
fn info_warning_at_70_percent() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(70_000);
    let warning = health.check_warning();
    assert!(warning.is_some());
    let w = warning.unwrap();
    assert_eq!(w.severity, WarningSeverity::Info);
    assert!(w.message.contains("70%"), "got: {}", w.message);
}

#[test]
fn warning_at_85_percent() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(85_000);
    let warning = health.check_warning();
    assert!(warning.is_some());
    let w = warning.unwrap();
    assert_eq!(w.severity, WarningSeverity::Warning);
    assert!(w.message.contains("85%"), "got: {}", w.message);
    assert!(
        w.message.contains("consider starting a new session"),
        "got: {}",
        w.message
    );
}

#[test]
fn critical_warning_at_95_percent() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(96_000);
    let warning = health.check_warning();
    assert!(warning.is_some());
    let w = warning.unwrap();
    assert_eq!(w.severity, WarningSeverity::Critical);
    assert!(w.message.contains("96%"), "got: {}", w.message);
}

#[test]
fn warning_issued_once_per_severity() {
    let mut health = ContextHealth::new(100_000);
    health.set_conversation_tokens(70_000);

    // First check — should get info warning
    let w1 = health.check_warning();
    assert!(w1.is_some());
    assert_eq!(w1.unwrap().severity, WarningSeverity::Info);

    // Second check at same level — no warning
    assert!(health.check_warning().is_none());

    // Increase to warning level
    health.set_conversation_tokens(85_000);
    let w2 = health.check_warning();
    assert!(w2.is_some());
    assert_eq!(w2.unwrap().severity, WarningSeverity::Warning);

    // No repeat
    assert!(health.check_warning().is_none());
}

#[test]
fn critical_warning_skips_lower_severities() {
    let mut health = ContextHealth::new(100_000);
    // Jump straight to critical without passing through info/warning
    health.set_conversation_tokens(96_000);
    let w = health.check_warning();
    assert!(w.is_some());
    assert_eq!(w.unwrap().severity, WarningSeverity::Critical);

    // Now lower severities should also be marked as issued
    health.set_conversation_tokens(70_000);
    assert!(health.check_warning().is_none());
}

// --- tool result tracking tests ---

#[test]
fn record_tool_result() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    assert_eq!(health.tool_results.len(), 1);
    assert_eq!(health.tool_results[0].tool_name, "Read");
    assert_eq!(health.tool_results[0].tokens, 5_000);
    assert_eq!(health.tool_results[0].turns_since_referenced, 0);
}

#[test]
fn advance_turn_increments_staleness() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    health.record_tool_result("Bash", 3_000);
    health.advance_turn();
    health.advance_turn();
    assert_eq!(health.tool_results[0].turns_since_referenced, 2);
    assert_eq!(health.tool_results[1].turns_since_referenced, 2);
}

#[test]
fn mark_referenced_resets_staleness() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    health.advance_turn();
    health.advance_turn();
    health.mark_referenced("Read");
    assert_eq!(health.tool_results[0].turns_since_referenced, 0);
}

// --- compaction suggestion tests ---

#[test]
fn no_suggestions_when_no_tool_results() {
    let health = ContextHealth::new(200_000);
    let suggestions = health.compaction_suggestions(1_000, 3);
    assert!(suggestions.is_empty());
}

#[test]
fn no_suggestions_when_results_fresh() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    // Only 1 turn old, threshold is 3
    health.advance_turn();
    let suggestions = health.compaction_suggestions(1_000, 3);
    assert!(suggestions.is_empty());
}

#[test]
fn no_suggestions_when_results_small() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 500);
    for _ in 0..5 {
        health.advance_turn();
    }
    // Small result (500 < 1000 threshold)
    let suggestions = health.compaction_suggestions(1_000, 3);
    assert!(suggestions.is_empty());
}

#[test]
fn suggests_compacting_large_stale_results() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    health.record_tool_result("Bash", 8_000);
    health.record_tool_result("scope_search", 500); // too small
    for _ in 0..5 {
        health.advance_turn();
    }

    let suggestions = health.compaction_suggestions(1_000, 3);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].item_count, 2);
    assert_eq!(suggestions[0].estimated_savings, 13_000);
    assert!(
        suggestions[0].description.contains("2 large tool results"),
        "got: {}",
        suggestions[0].description
    );
}

#[test]
fn singular_suggestion_for_one_result() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 12_000);
    for _ in 0..4 {
        health.advance_turn();
    }

    let suggestions = health.compaction_suggestions(1_000, 3);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].item_count, 1);
    assert!(
        suggestions[0].description.contains("1 large tool result "),
        "got: {}",
        suggestions[0].description
    );
}

#[test]
fn referenced_results_excluded_from_suggestions() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    health.record_tool_result("Bash", 8_000);
    for _ in 0..5 {
        health.advance_turn();
    }
    // Mark one as recently referenced
    health.mark_referenced("Read");

    let suggestions = health.compaction_suggestions(1_000, 3);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].item_count, 1); // Only Bash
    assert_eq!(suggestions[0].estimated_savings, 8_000);
}

#[test]
fn suggestion_description_includes_token_savings() {
    let mut health = ContextHealth::new(200_000);
    health.record_tool_result("Read", 5_000);
    for _ in 0..4 {
        health.advance_turn();
    }

    let suggestions = health.compaction_suggestions(1_000, 3);
    assert!(
        suggestions[0].description.contains("5.0K tokens"),
        "got: {}",
        suggestions[0].description
    );
}
