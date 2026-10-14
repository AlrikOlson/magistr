use super::*;
#[test]
fn formatting_is_enabled_by_default() {
    assert!(!MagistrEnv::default().no_autoformat);
}
