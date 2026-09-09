use super::*;

#[test]
fn severity_labels() {
    assert_eq!(Severity::Info.label(), "info");
    assert_eq!(Severity::Warning.label(), "warning");
    assert_eq!(Severity::Error.label(), "error");
}
