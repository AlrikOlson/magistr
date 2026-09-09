use super::*;

#[test]
fn default_is_acp() {
    assert_eq!(Backend::default(), Backend::Acp);
}

#[test]
fn acp_supports_cancel() {
    assert!(Backend::Acp.supports_cancel());
}

#[test]
fn acp_label() {
    assert_eq!(Backend::Acp.label(), "acp");
}

#[test]
fn all_compiled_contains_acp() {
    let all = Backend::all_compiled();
    assert!(all.contains(&Backend::Acp));
}

#[test]
fn capabilities_feature_list_has_eight_entries() {
    let caps = Backend::Acp.capabilities();
    assert_eq!(caps.feature_list().len(), 8);
}

#[test]
fn acp_capabilities_all_true() {
    let caps = Backend::Acp.capabilities();
    for (_, supported) in caps.feature_list() {
        assert!(supported);
    }
}
