use super::VERSION;

#[test]
fn version_is_reported() {
    assert!(!VERSION.is_empty());
}
