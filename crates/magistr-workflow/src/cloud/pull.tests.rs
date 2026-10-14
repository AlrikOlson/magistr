use super::max_watermark;
use serde_json::json;

#[test]
fn max_watermark_prefers_updated_and_takes_the_newest() {
    let envs = vec![
        json!({"created": "2026-06-01T00:00:00Z"}),
        json!({"created": "2026-06-01T00:00:00Z", "updated": "2026-06-11T09:00:00Z"}),
        json!({"updated": "2026-06-10T00:00:00Z"}),
    ];
    assert_eq!(
        max_watermark(&envs).as_deref(),
        Some("2026-06-11T09:00:00Z")
    );
}

#[test]
fn max_watermark_empty_is_none() {
    assert_eq!(max_watermark(&[]), None);
}
