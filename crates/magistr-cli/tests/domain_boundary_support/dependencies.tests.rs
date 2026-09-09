use serde_json::{Value, json};

use super::dependencies::check;

fn dependency(name: &str) -> Value {
    json!({
        "name": name, "kind": null, "optional": false, "target": null, "rename": null,
        "source": "registry+https://github.com/rust-lang/crates.io-index",
        "uses_default_features": false, "features": ["alloc", "derive"]
    })
}

fn metadata() -> Value {
    let registry = "registry+https://github.com/rust-lang/crates.io-index";
    json!({
        "workspace_root": "/project",
        "workspace_members": ["domain"],
        "packages": [
            {"id": "domain", "name": "magistr-domain", "source": null, "features": {},
             "manifest_path": "/project/crates/magistr-domain/Cargo.toml",
             "targets": [{"kind": ["lib"], "src_path": "/project/crates/magistr-domain/src/lib.rs"}],
             "dependencies": [dependency("serde")]},
            {"id": "serde", "name": "serde", "source": registry,
             "dependencies": [dependency("serde_core"), dependency("serde_derive")]},
            {"id": "serde_core", "name": "serde_core", "source": registry, "dependencies": []},
            {"id": "serde_derive", "name": "serde_derive", "source": registry,
             "dependencies": [dependency("syn"), dependency("quote"), dependency("proc-macro2")]},
            {"id": "syn", "name": "syn", "source": registry, "dependencies": [dependency("unicode-ident")]},
            {"id": "quote", "name": "quote", "source": registry, "dependencies": [dependency("proc-macro2")]},
            {"id": "proc-macro2", "name": "proc-macro2", "source": registry, "dependencies": [dependency("unicode-ident")]},
            {"id": "unicode-ident", "name": "unicode-ident", "source": registry, "dependencies": []}
        ]
    })
}

fn development_metadata() -> Value {
    let mut fixture = metadata();
    let mut development_dependency = dependency("serde_json");
    development_dependency["kind"] = json!("dev");
    development_dependency["uses_default_features"] = json!(true);
    development_dependency["features"] = json!([]);
    fixture["packages"][0]["dependencies"]
        .as_array_mut()
        .expect("domain dependencies")
        .push(development_dependency);
    for (name, children) in [
        (
            "serde_json",
            vec!["indexmap", "itoa", "memchr", "serde", "serde_core", "zmij"],
        ),
        (
            "indexmap",
            vec!["equivalent", "hashbrown", "serde", "serde_core"],
        ),
        ("itoa", vec![]),
        ("memchr", vec![]),
        ("zmij", vec![]),
        ("equivalent", vec![]),
        ("hashbrown", vec![]),
    ] {
        fixture["packages"]
            .as_array_mut()
            .expect("packages")
            .push(json!({
                "id": name, "name": name,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "dependencies": children.into_iter().map(dependency).collect::<Vec<_>>()
            }));
    }
    fixture["resolve"] = json!({"nodes": fixture["packages"].as_array().expect("packages")
        .iter().map(|package| json!({
            "id": package["id"],
            "deps": package["dependencies"].as_array().expect("dependencies")
                .iter().map(|dependency| json!({
                    "pkg": dependency["name"], "name": dependency["name"],
                    "dep_kinds": [{"kind": dependency["kind"], "target": dependency["target"]}]
                })).collect::<Vec<_>>()
        })).collect::<Vec<_>>()});
    fixture
}

#[test]
fn dependency_guard_accepts_the_explicit_serialization_closure() {
    assert_eq!(check(&metadata()), Vec::<String>::new());
    assert_eq!(check(&development_metadata()), Vec::<String>::new());
}

#[test]
fn dependency_guard_checks_allowed_serde_declaration_fields_independently() {
    for (field, value) in [
        ("optional", json!(true)),
        ("target", json!("cfg(windows)")),
        ("kind", json!("build")),
        ("rename", json!("pure_helper")),
    ] {
        let mut fixture = metadata();
        fixture["packages"][0]["dependencies"][0][field] = value;
        assert!(
            check(&fixture)
                .iter()
                .any(|error| error.contains("dependency serde is outside")),
            "accepted mutated serde field {field}"
        );
    }
}

#[test]
fn dependency_guard_rejects_patched_development_packages_and_effectful_children() {
    let mut patched = development_metadata();
    let package = patched["packages"]
        .as_array_mut()
        .expect("packages")
        .iter_mut()
        .find(|package| package["name"] == "serde_json")
        .expect("serde_json");
    package["source"] = Value::Null;
    assert!(
        check(&patched)
            .iter()
            .any(|error| error.contains("development package serde_json must come from crates.io"))
    );

    // The edge's alias is irrelevant: resolve its actual package ID and name.
    let mut effectful = development_metadata();
    effectful["packages"]
        .as_array_mut()
        .expect("packages")
        .push(json!({
            "id": "effect-package-id", "name": "tokio",
            "source": "registry+https://github.com/rust-lang/crates.io-index", "dependencies": []
        }));
    let serde_json = effectful["resolve"]["nodes"]
        .as_array_mut()
        .expect("nodes")
        .iter_mut()
        .find(|node| node["id"] == "serde_json")
        .expect("serde_json node");
    serde_json["deps"]
        .as_array_mut()
        .expect("edges")
        .push(json!({
            "pkg": "effect-package-id", "name": "pure_helper",
            "dep_kinds": [{"kind": null, "target": null}]
        }));
    assert!(
        check(&effectful)
            .iter()
            .any(|error| error.contains("serde_json -> tokio"))
    );
}

#[test]
fn dependency_guard_rejects_development_feature_and_provenance_escapes() {
    let mut fixture = development_metadata();
    fixture["packages"][0]["dependencies"][1]["features"] = json!(["preserve_order"]);
    assert!(
        check(&fixture)
            .iter()
            .any(|error| error.contains("dependency serde_json is outside"))
    );
    let mut fixture = development_metadata();
    let leaf = fixture["packages"]
        .as_array_mut()
        .expect("packages")
        .iter_mut()
        .find(|package| package["name"] == "itoa")
        .expect("itoa");
    leaf["source"] = Value::Null;
    assert!(
        check(&fixture)
            .iter()
            .any(|error| error.contains("development package itoa must come from crates.io"))
    );
    let mut fixture = development_metadata();
    fixture["resolve"] = Value::Null;
    assert!(
        check(&fixture)
            .iter()
            .any(|error| error.contains("resolution is missing"))
    );
}

#[test]
fn dependency_guard_rejects_every_effect_category_even_under_an_alias() {
    for name in [
        "tokio",
        "reqwest",
        "rusqlite",
        "magistr-store",
        "magistr-engine",
        "magistr-tui",
        "magistr-desktop",
        "libc",
    ] {
        let mut fixture = metadata();
        let mut effect = dependency(name);
        effect["rename"] = json!("pure_helper");
        fixture["packages"][0]["dependencies"]
            .as_array_mut()
            .expect("dependencies")
            .push(effect);
        assert!(
            check(&fixture).iter().any(|error| error.contains(name)),
            "accepted {name}"
        );
    }
}

#[test]
fn dependency_guard_rejects_optional_target_specific_build_and_transitive_edges() {
    for (field, value) in [
        ("optional", json!(true)),
        ("target", json!("cfg(windows)")),
        ("kind", json!("build")),
    ] {
        let mut fixture = metadata();
        let mut effect = dependency("effect_helper");
        effect[field] = value;
        fixture["packages"][0]["dependencies"]
            .as_array_mut()
            .expect("dependencies")
            .push(effect.clone());
        assert!(
            check(&fixture)
                .iter()
                .any(|error| error.contains("effect_helper"))
        );
        let mut transitive = metadata();
        transitive["packages"][5]["dependencies"]
            .as_array_mut()
            .expect("dependencies")
            .push(effect);
        assert!(
            check(&transitive)
                .iter()
                .any(|error| error.contains("quote -> effect_helper"))
        );
    }
}

#[test]
fn dependency_guard_rejects_std_features_local_patches_and_build_targets() {
    let mut fixture = metadata();
    fixture["packages"][0]["dependencies"][0]["uses_default_features"] = json!(true);
    assert!(!check(&fixture).is_empty());
    let mut fixture = metadata();
    fixture["packages"][0]["features"] = json!({"escape": ["serde/std"]});
    assert!(!check(&fixture).is_empty());
    let mut fixture = metadata();
    fixture["packages"][1]["source"] = Value::Null;
    assert!(
        check(&fixture)
            .iter()
            .any(|error| error.contains("must come from crates.io"))
    );
    let mut fixture = metadata();
    fixture["packages"][0]["targets"]
        .as_array_mut()
        .expect("targets")
        .push(json!({"kind": ["custom-build"], "src_path": "/project/effects.rs"}));
    assert!(
        check(&fixture)
            .iter()
            .any(|error| error.contains("build scripts"))
    );
}
