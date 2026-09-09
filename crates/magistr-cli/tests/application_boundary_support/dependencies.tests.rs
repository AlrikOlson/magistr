use serde_json::{Value, json};

use super::dependencies::check;

fn metadata() -> Value {
    json!({
        "workspace_root": "/project",
        "workspace_members": ["application", "domain"],
        "packages": [
            {"id": "application", "name": "magistr-application", "source": null, "features": {},
             "manifest_path": "/project/crates/magistr-application/Cargo.toml",
             "targets": [{"kind": ["lib"], "src_path": "/project/crates/magistr-application/src/lib.rs"}],
             "dependencies": [{"name": "magistr-domain", "kind": null, "optional": false,
                "target": null, "rename": null, "source": null, "registry": null,
                "path": "/project/crates/magistr-domain", "features": [], "uses_default_features": true}]},
            {"id": "domain", "name": "magistr-domain", "source": null,
             "manifest_path": "/project/crates/magistr-domain/Cargo.toml"}
        ],
        "resolve": {"nodes": [{"id": "application", "deps": [{"pkg": "domain",
            "name": "magistr_domain", "dep_kinds": [{"kind": null, "target": null}]}]}]}
    })
}

#[test]
fn dependency_guard_accepts_only_the_local_domain_edge() {
    assert_eq!(check(&metadata()), Vec::<String>::new());
}

#[test]
fn dependency_guard_rejects_every_adapter_category_and_alias() {
    for name in [
        "tokio",
        "reqwest",
        "libc",
        "magistr-core",
        "magistr-engine",
        "magistr-store",
        "magistr-protocol",
        "magistr-mcp",
        "rmcp",
        "magistr-acp",
        "magistr-cockpit",
        "magistr-tui",
        "magistr-desktop",
        "ratatui",
        "crossterm",
        "serde",
    ] {
        let mut fixture = metadata();
        let mut adapter = fixture["packages"][0]["dependencies"][0].clone();
        adapter["name"] = json!(name);
        adapter["rename"] = json!("contract_helper");
        fixture["packages"][0]["dependencies"]
            .as_array_mut()
            .expect("dependencies")
            .push(adapter);
        assert!(
            check(&fixture).iter().any(|error| error.contains(name)),
            "accepted {name}"
        );
    }
}

#[test]
fn dependency_guard_checks_each_domain_declaration_capability() {
    for (field, value) in [
        ("kind", json!("dev")),
        ("kind", json!("build")),
        ("optional", json!(true)),
        ("target", json!("cfg(any())")),
        ("rename", json!("contract_helper")),
        (
            "source",
            json!("registry+https://github.com/rust-lang/crates.io-index"),
        ),
        ("registry", json!("https://example.invalid/index")),
        ("path", json!("/project/effectful-domain")),
        ("features", json!(["adapter"])),
        ("uses_default_features", json!(false)),
    ] {
        let mut fixture = metadata();
        fixture["packages"][0]["dependencies"][0][field] = value;
        assert!(
            check(&fixture)
                .iter()
                .any(|error| error.contains("outside the local domain contract")),
            "accepted changed {field}"
        );
    }
    let mut fixture = metadata();
    fixture["packages"][0]["dependencies"] = json!([]);
    assert!(!check(&fixture).is_empty());
}

#[test]
fn dependency_guard_rejects_build_targets_features_and_alternate_roots() {
    for (field, value) in [
        ("features", json!({"adapter": ["magistr-domain/adapter"]})),
        (
            "manifest_path",
            json!("/project/effectful-application/Cargo.toml"),
        ),
        (
            "source",
            json!("registry+https://github.com/rust-lang/crates.io-index"),
        ),
        (
            "targets",
            json!([{"kind": ["lib"], "src_path": "/project/effectful.rs"}]),
        ),
        (
            "targets",
            json!([
                {"kind": ["lib"], "src_path": "/project/crates/magistr-application/src/lib.rs"},
                {"kind": ["custom-build"], "src_path": "/project/host.rs"}
            ]),
        ),
    ] {
        let mut fixture = metadata();
        fixture["packages"][0][field] = value;
        assert!(!check(&fixture).is_empty(), "accepted changed {field}");
    }
}

#[test]
fn dependency_guard_rejects_resolution_and_workspace_substitutions() {
    for (pointer, value) in [
        ("/workspace_members", json!(["domain"])),
        ("/workspace_members", json!(["application"])),
        (
            "/packages/1/source",
            json!("registry+https://github.com/rust-lang/crates.io-index"),
        ),
        (
            "/packages/1/manifest_path",
            json!("/project/other/Cargo.toml"),
        ),
        ("/resolve", Value::Null),
        ("/resolve/nodes/0/deps/0/pkg", json!("adapter")),
        ("/resolve/nodes/0/deps/0/name", json!("contract_helper")),
        ("/resolve/nodes/0/deps/0/dep_kinds/0/kind", json!("build")),
        (
            "/resolve/nodes/0/deps/0/dep_kinds/0/target",
            json!("cfg(windows)"),
        ),
    ] {
        let mut fixture = metadata();
        *fixture.pointer_mut(pointer).expect("fixture field") = value;
        assert!(!check(&fixture).is_empty(), "accepted changed {pointer}");
    }
}
