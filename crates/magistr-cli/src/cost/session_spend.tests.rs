use super::*;
use serde_json::json;

#[test]
fn builders_assemble_a_full_record() {
    let spend = SessionSpend::new("sess-1", Some(1.25))
        .with_run(4, 30_000)
        .with_model("claude-opus-5")
        .with_tokens(TokenBreakdown {
            input_tokens: 100,
            output_tokens: 50,
            ..TokenBreakdown::default()
        });

    assert_eq!(spend.session_id, "sess-1");
    assert_eq!(spend.cost_usd, Some(1.25));
    assert_eq!(spend.turns, 4);
    assert_eq!(spend.duration_ms, 30_000);
    assert_eq!(spend.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(spend.tokens.total(), 150);
}

#[test]
fn usage_is_read_with_cache_kept_separate_from_input() {
    let usage = json!({
        "input_tokens": 120,
        "output_tokens": 40,
        "cache_read_input_tokens": 9_000,
        "cache_creation_input_tokens": 300,
    });
    let tokens = SessionSpend::tokens_from_usage(&usage);
    assert_eq!(tokens.input_tokens, 120);
    assert_eq!(tokens.output_tokens, 40);
    assert_eq!(tokens.cache_read_tokens, 9_000);
    assert_eq!(tokens.cache_write_tokens, 300);
    assert_eq!(tokens.total(), 9_460);
}

#[test]
fn a_usage_object_missing_every_field_reads_as_zero() {
    let tokens = SessionSpend::tokens_from_usage(&json!({}));
    assert_eq!(tokens.total(), 0);
    let tokens = SessionSpend::tokens_from_usage(&json!("not an object"));
    assert_eq!(tokens.total(), 0);
}

#[test]
fn only_an_absent_session_without_measurements_is_empty() {
    assert!(SessionSpend::default().is_empty());
    assert!(!SessionSpend::new("sess-1", Some(0.0)).is_empty());
    assert!(!SessionSpend::new("sess-1", None).is_empty());
    assert!(!SessionSpend::new("sess-1", Some(0.01)).is_empty());
    assert!(
        !SessionSpend::new("sess-1", Some(0.0))
            .with_tokens(TokenBreakdown {
                input_tokens: 1,
                ..TokenBreakdown::default()
            })
            .is_empty()
    );
}
