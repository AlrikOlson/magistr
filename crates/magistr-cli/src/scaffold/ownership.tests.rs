use super::*;

#[test]
fn ownership_is_judged_from_the_policy_and_the_manifest_record() {
    let mut manifest = ScaffoldManifest::new();
    manifest.record("recorded.md", "as written");

    let judge =
        |policy, path, existing: Option<&str>| Ownership::judge(policy, path, existing, &manifest);

    assert_eq!(
        judge(UpdatePolicy::PreserveCustomized, "recorded.md", None),
        Ownership::Unclaimed
    );
    assert_eq!(
        judge(UpdatePolicy::NeverOverwrite, "CLAUDE.md", Some("anything")),
        Ownership::User
    );
    assert_eq!(
        judge(UpdatePolicy::Overwrite, "tools.md", Some("anything")),
        Ownership::Internal
    );
    assert_eq!(
        judge(
            UpdatePolicy::PreserveCustomized,
            "recorded.md",
            Some("as written")
        ),
        Ownership::Generated
    );
    assert_eq!(
        judge(
            UpdatePolicy::PreserveCustomized,
            "recorded.md",
            Some("edited")
        ),
        Ownership::Customized
    );
    assert_eq!(
        judge(
            UpdatePolicy::PreserveCustomized,
            "unrecorded.md",
            Some("theirs")
        ),
        Ownership::User
    );
}
