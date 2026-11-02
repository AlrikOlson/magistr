use super::*;

#[test]
fn default_is_workflow() {
    assert_eq!(PlanBackend::default(), PlanBackend::Native);
    assert!(PlanBackend::Native.is_default());
    assert!(!PlanBackend::Yaml.is_default());
    assert!(!PlanBackend::Yaml.is_native());
    assert!(PlanBackend::Native.is_native());
}

#[test]
fn wire_form_is_kebab_case() {
    assert_eq!(
        serde_json::to_string(&PlanBackend::Native).expect("ser"),
        "\"native\""
    );
    let parsed: PlanBackend = serde_json::from_str("\"yaml\"").expect("de");
    assert_eq!(parsed, PlanBackend::Yaml);
    assert_eq!(PlanBackend::Native.as_str(), "native");
    assert!(serde_json::from_str::<PlanBackend>("\"markdown\"").is_err());
}
