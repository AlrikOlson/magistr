use super::*;

#[test]
fn matches_javascript_to_iso_string() {
    assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(utc_timestamp(1_787_356_800_000), "2026-08-22T00:00:00.000Z");
    assert_eq!(utc_timestamp(1_755_820_800_000), "2025-08-22T00:00:00.000Z");
    assert_eq!(utc_timestamp(1_700_000_002_000), "2023-11-14T22:13:22.000Z");
    assert_eq!(utc_timestamp(951_782_400_123), "2000-02-29T00:00:00.123Z");
}
