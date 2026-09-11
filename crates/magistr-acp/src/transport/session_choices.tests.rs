use super::nearest_model;

fn offered(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn an_offered_id_is_taken_verbatim() {
    let models = offered(&["default", "opus[1m]", "claude-fable-5-1[1m]", "sonnet"]);
    assert_eq!(
        nearest_model(&models, "claude-fable-5-1[1m]").as_deref(),
        Some("claude-fable-5-1[1m]")
    );
    assert_eq!(nearest_model(&models, "sonnet").as_deref(), Some("sonnet"));
}

#[test]
fn a_stale_family_id_resolves_to_the_newer_release_with_the_same_context_marker() {
    let models = offered(&[
        "default",
        "opus[1m]",
        "claude-fable-5-1",
        "claude-fable-5-1[1m]",
        "sonnet",
    ]);
    assert_eq!(
        nearest_model(&models, "claude-fable-5[1m]").as_deref(),
        Some("claude-fable-5-1[1m]")
    );
    assert_eq!(
        nearest_model(&models, "claude-fable-5").as_deref(),
        Some("claude-fable-5-1")
    );
    // A missing marker match still lands in the family, in offered order.
    assert_eq!(
        nearest_model(&offered(&["claude-fable-5-1[1m]"]), "claude-fable-5").as_deref(),
        Some("claude-fable-5-1[1m]")
    );
}

#[test]
fn unrelated_ids_and_prefix_collisions_are_not_a_family() {
    let models = offered(&["claude-fable-5-1[1m]", "claude-opus-5", "sonnet"]);
    assert_eq!(nearest_model(&models, "claude-fable-4[1m]"), None);
    assert_eq!(nearest_model(&models, "claude-fab"), None);
    assert_eq!(nearest_model(&models, "gpt-9"), None);
    assert_eq!(nearest_model(&models, "[1m]"), None);
}
