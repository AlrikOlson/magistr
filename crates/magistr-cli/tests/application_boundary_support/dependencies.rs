use std::path::Path;

use serde_json::{Value, json};

/// Pin declarations as well as resolution: disabled optional dependencies and
/// target-specific aliases must not smuggle adapter capabilities into contracts.
/// The domain's own architecture gate enforces its serialization-only closure.
pub(super) fn check(metadata: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(packages) = metadata["packages"].as_array() else {
        return vec!["Cargo metadata has no packages".into()];
    };
    let Some(application) = packages
        .iter()
        .find(|package| package["name"] == "magistr-application")
    else {
        return vec!["magistr-application must be a workspace package".into()];
    };
    let Some(workspace) = metadata["workspace_root"].as_str().map(Path::new) else {
        return vec!["Cargo metadata has no workspace root".into()];
    };
    let root = workspace.join("crates/magistr-application");
    let domain_root = workspace.join("crates/magistr-domain");
    if application["source"] != Value::Null
        || application["manifest_path"].as_str().map(Path::new)
            != Some(root.join("Cargo.toml").as_path())
        || !metadata["workspace_members"]
            .as_array()
            .is_some_and(|members| members.contains(&application["id"]))
    {
        errors.push("application must be the crates/magistr-application workspace package".into());
    }
    if !application["targets"].as_array().is_some_and(|targets| {
        targets.len() == 1
            && targets[0]["kind"] == json!(["lib"])
            && targets[0]["src_path"].as_str().map(Path::new)
                == Some(root.join("src/lib.rs").as_path())
    }) {
        errors.push("application must have only its src/lib.rs target, without build scripts or alternate roots".into());
    }
    if !application["features"].as_object().is_some_and(|features| {
        features.is_empty() || (features.len() == 1 && features.get("default") == Some(&json!([])))
    }) {
        errors.push("application features must not enable additional capabilities".into());
    }
    match application["dependencies"].as_array() {
        Some(dependencies) => {
            if dependencies.len() != 1 {
                errors.push("application must have exactly one dependency: magistr-domain".into());
            }
            for dependency in dependencies {
                if dependency["name"] != "magistr-domain"
                    || !dependency["kind"].is_null()
                    || dependency["optional"] != false
                    || !dependency["target"].is_null()
                    || !dependency["rename"].is_null()
                    || !dependency["source"].is_null()
                    || !dependency["registry"].is_null()
                    || dependency["path"].as_str().map(Path::new) != Some(domain_root.as_path())
                    || dependency["features"] != json!([])
                    || dependency["uses_default_features"] != true
                {
                    errors.push(format!(
                        "application dependency {} is outside the local domain contract",
                        dependency["name"]
                    ));
                }
            }
        }
        None => errors.push("application dependency declarations are missing".into()),
    }
    let Some(domain) = packages
        .iter()
        .find(|package| package["name"] == "magistr-domain")
    else {
        errors.push("application dependency resolution has no magistr-domain".into());
        return errors;
    };
    if !domain["source"].is_null()
        || domain["manifest_path"].as_str().map(Path::new)
            != Some(domain_root.join("Cargo.toml").as_path())
        || !metadata["workspace_members"]
            .as_array()
            .is_some_and(|members| members.contains(&domain["id"]))
    {
        errors.push("application must resolve the local magistr-domain workspace package".into());
    }
    let edges = metadata["resolve"]["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["id"] == application["id"]))
        .and_then(|node| node["deps"].as_array());
    if !edges.is_some_and(|edges| {
        edges.len() == 1
            && edges[0]["pkg"] == domain["id"]
            && edges[0]["name"] == "magistr_domain"
            && edges[0]["dep_kinds"] == json!([{"kind": null, "target": null}])
    }) {
        errors.push("application must resolve only its unaliased domain dependency".into());
    }
    errors
}
