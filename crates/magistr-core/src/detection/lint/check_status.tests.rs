use super::*;

#[test]
fn check_status_labels() {
    assert_eq!(CheckStatus::Pass.label(), "pass");
    assert_eq!(CheckStatus::Warn.label(), "warn");
    assert_eq!(CheckStatus::Fail.label(), "fail");
}

#[test]
fn check_status_icons() {
    assert_eq!(CheckStatus::Pass.icon(), "✓");
    assert_eq!(CheckStatus::Warn.icon(), "⚠");
    assert_eq!(CheckStatus::Fail.icon(), "✗");
}

#[test]
fn check_status_display() {
    assert_eq!(CheckStatus::Pass.to_string(), "pass");
}
