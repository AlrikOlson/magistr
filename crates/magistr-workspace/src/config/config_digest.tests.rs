use super::*;
use crate::config::ResolveOptions;

fn rig() -> (tempfile::TempDir, ResolveOptions) {
    let temp = tempfile::tempdir().unwrap();
    let mut options = ResolveOptions::new(temp.path());
    options.env.insert(
        "MAGISTR_WORKSPACE_ORG_POLICY".into(),
        temp.path().join("org.toml").display().to_string(),
    );
    options.env.insert(
        "MAGISTR_WORKSPACE_USER_CONFIG".into(),
        temp.path().join("user.toml").display().to_string(),
    );
    (temp, options)
}

#[test]
fn the_external_layers_are_the_org_policy_and_the_user_layer_never_the_workspace() {
    let (temp, options) = rig();
    let external = options.external_layer_paths();
    assert_eq!(
        external
            .iter()
            .map(|(layer, path)| (*layer, path.clone()))
            .collect::<Vec<_>>(),
        vec![
            (ConfigLayer::OrgPolicy, temp.path().join("org.toml")),
            (ConfigLayer::User, temp.path().join("user.toml")),
        ]
    );
}

#[test]
fn a_missing_layer_is_absent_and_differs_from_an_empty_file() {
    let (temp, options) = rig();
    let absent = ConfigDigest::observe(&options.external_layer_paths()).unwrap();
    assert_eq!(
        absent.layers,
        vec![
            ConfigLayerDigest {
                layer: ConfigLayer::OrgPolicy,
                present: false,
                digest: String::new(),
            },
            ConfigLayerDigest {
                layer: ConfigLayer::User,
                present: false,
                digest: String::new(),
            },
        ]
    );
    assert_eq!(
        absent.digest(),
        ConfigDigest::observe(&options.external_layer_paths())
            .unwrap()
            .digest(),
        "deterministic"
    );

    std::fs::write(temp.path().join("org.toml"), "").unwrap();
    let empty = ConfigDigest::observe(&options.external_layer_paths()).unwrap();
    assert!(empty.layers[0].present);
    assert_ne!(absent.digest(), empty.digest(), "absent is not empty");
    assert_eq!(absent.changes_to(&empty), vec![ConfigLayer::OrgPolicy]);
    assert!(empty.changes_to(&empty).is_empty());
}

#[test]
fn content_alone_moves_the_digest_and_the_changed_layer_is_named() {
    let (temp, options) = rig();
    std::fs::write(temp.path().join("org.toml"), "[security]\nredaction=true\n").unwrap();
    std::fs::write(
        temp.path().join("user.toml"),
        "[telemetry]\nconsent='off'\n",
    )
    .unwrap();
    let before = ConfigDigest::observe(&options.external_layer_paths()).unwrap();

    std::fs::write(
        temp.path().join("user.toml"),
        "[telemetry]\nconsent='crash'\n",
    )
    .unwrap();
    let after = ConfigDigest::observe(&options.external_layer_paths()).unwrap();
    assert_ne!(before.digest(), after.digest());
    assert_eq!(before.changes_to(&after), vec![ConfigLayer::User]);
    assert_eq!(
        before.layers[0], after.layers[0],
        "the org layer is untouched"
    );

    // The same bytes elsewhere are the same effective layer.
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        elsewhere.path().join("policy.toml"),
        "[security]\nredaction=true\n",
    )
    .unwrap();
    std::fs::write(
        elsewhere.path().join("me.toml"),
        "[telemetry]\nconsent='crash'\n",
    )
    .unwrap();
    let moved = ConfigDigest::observe(&[
        (ConfigLayer::OrgPolicy, elsewhere.path().join("policy.toml")),
        (ConfigLayer::User, elsewhere.path().join("me.toml")),
    ])
    .unwrap();
    assert_eq!(
        moved.digest(),
        after.digest(),
        "paths never enter the digest"
    );

    let json = serde_json::to_string(&after).unwrap();
    assert!(!json.contains("consent"), "no layer content is serialized");
    assert!(!json.contains("user.toml"), "no layer path is serialized");
}

#[test]
fn an_unreadable_layer_refuses_the_observation_by_name() {
    let (temp, options) = rig();
    std::fs::create_dir_all(temp.path().join("org.toml")).unwrap();
    let refused = ConfigDigest::observe(&options.external_layer_paths()).unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("org-policy configuration layer"),
        "{refused}"
    );
}
