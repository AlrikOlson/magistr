use super::*;

#[test]
fn infer_researcher_coder() {
    let preset = OrchestrationPreset::infer(&["researcher", "coder"]);
    assert_eq!(preset, OrchestrationPreset::ResearchThenImplement);
}

#[test]
fn infer_coder_reviewer() {
    let preset = OrchestrationPreset::infer(&["coder", "reviewer"]);
    assert_eq!(preset, OrchestrationPreset::ReviewPair);
}

#[test]
fn infer_three_roles() {
    let preset = OrchestrationPreset::infer(&["researcher", "coder", "tester"]);
    assert_eq!(preset, OrchestrationPreset::ParallelTdd);
}

#[test]
fn infer_single_role_fallback() {
    let preset = OrchestrationPreset::infer(&["coder"]);
    assert_eq!(preset, OrchestrationPreset::ParallelTdd);
}

#[test]
fn infer_ambiguous_pair() {
    let preset = OrchestrationPreset::infer(&["researcher", "tester"]);
    assert_eq!(preset, OrchestrationPreset::ParallelTdd);
}

#[test]
fn infer_order_independent() {
    let a = OrchestrationPreset::infer(&["coder", "researcher"]);
    let b = OrchestrationPreset::infer(&["researcher", "coder"]);
    assert_eq!(a, b);
}

#[test]
fn display_matches_serde() {
    let preset = OrchestrationPreset::ResearchThenImplement;
    let json = serde_json::to_string(&preset).unwrap();
    assert_eq!(json, "\"research-then-implement\"");
    assert_eq!(preset.to_string(), "research-then-implement");
}

#[test]
fn serde_roundtrip() {
    for preset in [
        OrchestrationPreset::ResearchThenImplement,
        OrchestrationPreset::ReviewPair,
        OrchestrationPreset::ParallelTdd,
    ] {
        let json = serde_json::to_string(&preset).unwrap();
        let back: OrchestrationPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(back, preset);
    }
}

#[test]
fn infer_with_string_vec() {
    let roles: Vec<String> = vec!["researcher".into(), "coder".into()];
    let preset = OrchestrationPreset::infer(&roles);
    assert_eq!(preset, OrchestrationPreset::ResearchThenImplement);
}
