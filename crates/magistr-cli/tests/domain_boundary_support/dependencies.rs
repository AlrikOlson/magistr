use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// Inspect declarations as well as resolution: Cargo omits disabled optional
/// edges from its resolved graph, and aliases never change a package's `name`.
pub(super) fn check(metadata: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(packages) = metadata["packages"].as_array() else {
        return vec!["Cargo metadata has no packages".into()];
    };
    let Some(domain) = packages
        .iter()
        .find(|package| package["name"] == "magistr-domain")
    else {
        return vec!["magistr-domain must be a workspace package".into()];
    };
    if !metadata["workspace_members"]
        .as_array()
        .is_some_and(|members| members.contains(&domain["id"]))
    {
        errors.push("magistr-domain must be a workspace member".into());
    }
    let workspace = metadata["workspace_root"].as_str().map(Path::new);
    let expected_manifest = workspace.map(|root| root.join("crates/magistr-domain/Cargo.toml"));
    let expected_library = workspace.map(|root| root.join("crates/magistr-domain/src/lib.rs"));
    if expected_manifest.is_none()
        || domain["manifest_path"].as_str().map(Path::new) != expected_manifest.as_deref()
    {
        errors.push(
            "domain manifest must be crates/magistr-domain/Cargo.toml in this workspace".into(),
        );
    }
    let targets = domain["targets"].as_array();
    if !targets.is_some_and(|targets| {
        targets.len() == 1
            && targets[0]["kind"] == serde_json::json!(["lib"])
            && expected_library.is_some()
            && targets[0]["src_path"].as_str().map(Path::new) == expected_library.as_deref()
    }) {
        errors.push(
            "domain must have only its src/lib.rs target, without build scripts or alternate roots"
                .into(),
        );
    }
    if !domain["features"].as_object().is_some_and(|features| {
        features.is_empty()
            || (features.len() == 1 && features.get("default") == Some(&serde_json::json!([])))
    }) {
        errors.push("domain features must not enable additional dependency capabilities".into());
    }
    let Some(dependencies) = domain["dependencies"].as_array() else {
        return vec!["domain dependency declarations are missing".into()];
    };
    let mut normal_count = 0;
    for dependency in dependencies {
        let name = dependency["name"].as_str().unwrap_or("<missing>");
        let kind = dependency["kind"].as_str().unwrap_or("normal");
        let permitted = match kind {
            "normal" => {
                normal_count += 1;
                name == "serde"
                    && dependency["uses_default_features"] == false
                    && dependency["features"].as_array().is_some_and(|features| {
                        features.len() == 2
                            && features.contains(&Value::from("alloc"))
                            && features.contains(&Value::from("derive"))
                    })
            }
            "dev" => {
                name == "serde_json"
                    && dependency["uses_default_features"] == true
                    && dependency["features"] == serde_json::json!([])
            }
            _ => false,
        };
        if !permitted
            || dependency["optional"] != false
            || !dependency["target"].is_null()
            || !dependency["rename"].is_null()
            || dependency["source"] != CRATES_IO
        {
            errors.push(format!(
                "domain {kind} dependency {name} is outside the serde/alloc boundary"
            ));
        }
    }
    if normal_count != 1 {
        errors.push(
            "domain must have exactly one normal dependency: serde with alloc and derive".into(),
        );
    }

    let mut pending = vec!["serde".to_owned()];
    let mut visited = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let matching = packages
            .iter()
            .filter(|package| package["name"] == name)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            errors.push(format!("dependency closure has no metadata for {name}"));
        }
        for package in matching {
            if package["source"] != CRATES_IO {
                errors.push(format!(
                    "domain closure package {name} must come from crates.io"
                ));
            }
            let Some(declarations) = package["dependencies"].as_array() else {
                errors.push(format!("missing dependency declarations for {name}"));
                continue;
            };
            for dependency in declarations {
                if dependency["kind"] == "dev" {
                    continue;
                }
                let child = dependency["name"].as_str().unwrap_or("<missing>");
                if !allowed_children(&name).contains(&child) || dependency["source"] != CRATES_IO {
                    errors.push(format!(
                        "domain dependency closure forbids {name} -> {child} ({})",
                        dependency["kind"]
                    ));
                } else {
                    pending.push(child.to_owned());
                }
            }
        }
    }
    if dependencies
        .iter()
        .any(|dependency| dependency["name"] == "serde_json" && dependency["kind"] == "dev")
    {
        check_development_closure(metadata, packages, domain, &mut errors);
    }
    errors
}

// Dev-only serde_json enables std, so its actual package provenance and resolved
// capabilities must also be checked. The workspace unifies pure features such
// as preserve_order from other users; inspect package IDs rather than assuming
// the domain's empty feature list describes that shared graph. Production serde
// retains the stricter declaration walk above, including disabled optional edges.
fn check_development_closure(
    metadata: &Value,
    packages: &[Value],
    domain: &Value,
    errors: &mut Vec<String>,
) {
    let Some(nodes) = metadata["resolve"]["nodes"].as_array() else {
        errors.push("domain development dependency resolution is missing".into());
        return;
    };
    let Some(domain_node) = nodes.iter().find(|node| node["id"] == domain["id"]) else {
        errors.push("domain package is missing from dependency resolution".into());
        return;
    };
    let mut pending = domain_node["deps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|edge| {
            packages
                .iter()
                .any(|package| package["id"] == edge["pkg"] && package["name"] == "serde_json")
        })
        .filter_map(|edge| edge["pkg"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    if pending.is_empty() {
        errors.push("domain serde_json dependency is missing from resolution".into());
    }
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let Some(package) = packages.iter().find(|package| package["id"] == id) else {
            errors.push(format!("missing development package metadata for {id}"));
            continue;
        };
        let name = package["name"].as_str().unwrap_or("<missing>");
        if package["source"] != CRATES_IO {
            errors.push(format!(
                "domain development package {name} must come from crates.io"
            ));
        }
        let Some(edges) = nodes
            .iter()
            .find(|node| node["id"] == id)
            .and_then(|node| node["deps"].as_array())
        else {
            errors.push(format!("missing development dependency edges for {name}"));
            continue;
        };
        for edge in edges {
            if edge["dep_kinds"].as_array().is_some_and(|kinds| {
                !kinds.is_empty() && kinds.iter().all(|kind| kind["kind"] == "dev")
            }) {
                continue;
            }
            let Some(child) = packages.iter().find(|package| package["id"] == edge["pkg"]) else {
                errors.push(format!("missing resolved development child of {name}"));
                continue;
            };
            let child_name = child["name"].as_str().unwrap_or("<missing>");
            if !allowed_development_children(name).contains(&child_name) {
                errors.push(format!(
                    "domain development closure forbids {name} -> {child_name}"
                ));
            } else if let Some(id) = child["id"].as_str() {
                pending.push(id.to_owned());
            } else {
                errors.push(format!(
                    "missing resolved development package ID for {child_name}"
                ));
            }
        }
    }
}

fn allowed_development_children(name: &str) -> &'static [&'static str] {
    match name {
        "serde_json" => &["indexmap", "itoa", "memchr", "serde", "serde_core", "zmij"],
        "indexmap" => &["equivalent", "hashbrown", "serde", "serde_core"],
        // These are resolved leaves. Activating one of their optional dependency
        // features requires a deliberate boundary review, even via another user.
        "itoa" | "memchr" | "zmij" | "equivalent" | "hashbrown" => &[],
        _ => allowed_children(name),
    }
}

fn allowed_children(name: &str) -> &'static [&'static str] {
    match name {
        "serde" => &["serde_core", "serde_derive"],
        "serde_core" => &["serde_derive"],
        "serde_derive" => &["proc-macro2", "quote", "syn"],
        "syn" => &["proc-macro2", "quote", "unicode-ident"],
        "quote" => &["proc-macro2"],
        "proc-macro2" => &["unicode-ident"],
        _ => &[],
    }
}
