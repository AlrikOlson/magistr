use super::*;

#[test]
fn this_build_is_named_by_the_build_script_not_the_package_version_alone() {
    let build = BuildIdentity::this_build();
    assert_eq!(build.package(), env!("CARGO_PKG_VERSION"));
    assert_eq!(build.contract().len(), 16, "{}", build.contract());
    assert!(
        build.contract().bytes().all(|b| b.is_ascii_hexdigit()),
        "{}",
        build.contract()
    );
    assert!(
        matches!(build.profile(), "debug" | "release"),
        "{}",
        build.profile()
    );
    assert!(build.target().contains('-'), "{}", build.target());
    let text = build.to_string();
    for field in [
        build.package(),
        build.contract(),
        build.profile(),
        build.target(),
    ] {
        assert!(text.contains(field), "{text} names {field}");
    }
}

#[test]
fn the_rule_is_exact_build_and_a_refusal_names_both_identities() {
    let served = BuildIdentity::new(
        "0.1.0",
        "0011223344556677",
        "release",
        "aarch64-apple-darwin",
    );
    served
        .admits(&served.clone())
        .expect("an equal build attaches");
    let copied = BuildIdentity::new(
        "0.1.0",
        "0011223344556677",
        "release",
        "aarch64-apple-darwin",
    );
    served
        .admits(&copied)
        .expect("a copy of the same build is the same build");

    let other_checkout = BuildIdentity::new(
        "0.1.0",
        "8899aabbccddeeff",
        "release",
        "aarch64-apple-darwin",
    );
    let debug_of_the_same_tree =
        BuildIdentity::new("0.1.0", "0011223344556677", "debug", "aarch64-apple-darwin");
    let other_target = BuildIdentity::new(
        "0.1.0",
        "0011223344556677",
        "release",
        "x86_64-unknown-linux-gnu",
    );
    let other_version = BuildIdentity::new(
        "0.2.0",
        "0011223344556677",
        "release",
        "aarch64-apple-darwin",
    );
    for offered in [
        other_checkout,
        debug_of_the_same_tree,
        other_target,
        other_version,
    ] {
        let refusal = served
            .admits(&offered)
            .expect_err("equal package versions are not equal builds");
        assert_eq!(
            refusal,
            AttachRefusal::Build {
                served: Box::new(served.clone()),
                offered: Box::new(offered.clone()),
            }
        );
        let text = refusal.to_string();
        assert!(text.contains(&served.to_string()), "{text}");
        assert!(text.contains(&offered.to_string()), "{text}");
    }
}

#[test]
fn the_wire_spelling_is_four_named_fields() {
    let build = BuildIdentity::new("0.1.0", "0011223344556677", "debug", "aarch64-apple-darwin");
    let value = serde_json::to_value(&build).expect("serialize");
    assert_eq!(
        value,
        serde_json::json!({
            "package": "0.1.0",
            "contract": "0011223344556677",
            "profile": "debug",
            "target": "aarch64-apple-darwin",
        })
    );
    let back: BuildIdentity = serde_json::from_value(value).expect("deserialize");
    assert_eq!(back, build);
    // A protocol-3 build string is not an identity.
    assert!(serde_json::from_value::<BuildIdentity>(serde_json::json!("0.1.0")).is_err());
}
