use super::*;

#[test]
fn all_database_layers_parse() {
    for slug in database_slugs() {
        let result = load_database_layer(slug);
        assert!(
            result.is_ok(),
            "failed to parse database layer '{slug}': {:?}",
            result.err()
        );
        let layer = result.unwrap();
        assert!(layer.is_some(), "database layer '{slug}' returned None");
        let layer = layer.unwrap();
        assert_eq!(layer.layer_type, PresetLayerType::Database);
        assert_eq!(layer.name, *slug);
    }
}

#[test]
fn all_ci_layers_parse() {
    for slug in ci_slugs() {
        let result = load_ci_layer(slug);
        assert!(
            result.is_ok(),
            "failed to parse CI layer '{slug}': {:?}",
            result.err()
        );
        let layer = result.unwrap();
        assert!(layer.is_some(), "CI layer '{slug}' returned None");
        let layer = layer.unwrap();
        assert_eq!(layer.layer_type, PresetLayerType::CiCd);
    }
}

#[test]
fn all_deployment_layers_parse() {
    for slug in deployment_slugs() {
        let result = load_deployment_layer(slug);
        assert!(
            result.is_ok(),
            "failed to parse deployment layer '{slug}': {:?}",
            result.err()
        );
        let layer = result.unwrap();
        assert!(layer.is_some(), "deployment layer '{slug}' returned None");
        let layer = layer.unwrap();
        assert_eq!(layer.layer_type, PresetLayerType::Deployment);
    }
}

#[test]
fn unknown_database_slug_returns_none() {
    let result = load_database_layer("cassandra").unwrap();
    assert!(result.is_none());
}

#[test]
fn unknown_ci_slug_returns_none() {
    let result = load_ci_layer("jenkins").unwrap();
    assert!(result.is_none());
}

#[test]
fn unknown_deployment_slug_returns_none() {
    let result = load_deployment_layer("heroku").unwrap();
    assert!(result.is_none());
}

#[test]
fn database_layer_aliases_work() {
    // "sqlx" is an alias for "postgresql-sqlx"
    let layer = load_database_layer("sqlx").unwrap();
    assert!(layer.is_some());
    assert_eq!(layer.unwrap().name, "sqlx");

    // "mongodb" is an alias for "mongodb-mongoose"
    let layer = load_database_layer("mongodb").unwrap();
    assert!(layer.is_some());

    // "ef" is an alias for "entity-framework"
    let layer = load_database_layer("ef").unwrap();
    assert!(layer.is_some());
}

#[test]
fn ci_layer_aliases_work() {
    let layer = load_ci_layer("github").unwrap();
    assert!(layer.is_some());

    let layer = load_ci_layer("gha").unwrap();
    assert!(layer.is_some());

    let layer = load_ci_layer("gitlab").unwrap();
    assert!(layer.is_some());
}

#[test]
fn deployment_layer_aliases_work() {
    let layer = load_deployment_layer("k8s").unwrap();
    assert!(layer.is_some());

    let layer = load_deployment_layer("lambda").unwrap();
    assert!(layer.is_some());

    let layer = load_deployment_layer("fly").unwrap();
    assert!(layer.is_some());
}

#[test]
fn database_layers_have_extra_tools() {
    for slug in database_slugs() {
        let layer = load_database_layer(slug).unwrap().unwrap();
        assert!(
            !layer.overrides.extra_allowed_tools.is_empty(),
            "database layer '{slug}' has no extra_allowed_tools"
        );
    }
}

#[test]
fn ci_layers_have_extra_tools() {
    for slug in ci_slugs() {
        let layer = load_ci_layer(slug).unwrap().unwrap();
        assert!(
            !layer.overrides.extra_allowed_tools.is_empty(),
            "CI layer '{slug}' has no extra_allowed_tools"
        );
    }
}

#[test]
fn deployment_layers_have_extra_tools() {
    for slug in deployment_slugs() {
        let layer = load_deployment_layer(slug).unwrap().unwrap();
        assert!(
            !layer.overrides.extra_allowed_tools.is_empty(),
            "deployment layer '{slug}' has no extra_allowed_tools"
        );
    }
}
