use super::*;

const ICONS: &Icons = &Icons::UNICODE;

#[test]
fn a_led_runs_until_its_job_returns_then_reads_pass_or_fail() {
    let mut led = GateLed::new("check", JobId::new(4));
    assert_eq!(led.name(), "check");
    assert_eq!(led.job(), JobId::new(4));
    assert_eq!(led.passed(), None);
    assert_eq!(led.glyph(ICONS), "◌");
    assert_eq!(led.text(ICONS), "◌ check");
    led.settle(true);
    assert_eq!(led.passed(), Some(true));
    assert_eq!(led.text(ICONS), "✓ check");
    let mut failed = GateLed::new("lint", JobId::new(5));
    failed.settle(false);
    assert_eq!(failed.glyph(ICONS), "✗");
    assert_eq!(failed.text(ICONS), "✗ lint");
}
