use super::*;

#[test]
fn format_and_parse_round_trip_known_instants() {
    assert_eq!(HostClock::format(0), "1970-01-01T00:00:00Z");
    assert_eq!(HostClock::format(1_787_356_800), "2026-08-22T00:00:00Z");
    assert_eq!(
        HostClock::parse("2026-08-22T00:00:00Z"),
        Some(1_787_356_800)
    );
    assert_eq!(
        HostClock::parse("2026-08-22T12:00:00.000Z"),
        Some(1_787_400_000)
    );
    assert_eq!(
        HostClock::parse("2026-08-22T12:00:00+00:00"),
        Some(1_787_400_000)
    );
    for secs in [86_399, 951_782_400, 4_102_444_800, 1_709_251_199] {
        assert_eq!(HostClock::parse(&HostClock::format(secs)), Some(secs));
    }
    assert_eq!(HostClock::parse("not a time"), None);
    assert_eq!(HostClock::parse("2026-13-01T00:00:00Z"), None);
    assert!(HostClock::now().ends_with('Z'));
}
