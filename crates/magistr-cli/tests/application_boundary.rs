//! Application contracts may await injected ports, but cannot own adapters.
//! Inspect all source, including disabled cfg branches, and Cargo declarations.

#[path = "application_boundary_support/dependencies.rs"]
mod dependencies;
#[path = "domain_boundary_support/source.rs"]
mod source;

use std::path::Path;
use std::process::Command;

#[test]
fn application_sources_cannot_access_effects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../magistr-application");
    let errors = source::check_tree_with_async(&root, true);
    assert!(
        errors.is_empty(),
        "application source boundary:\n{}",
        errors.join("\n")
    );
}

fn workspace_metadata() -> serde_json::Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Resolution only needs this host; declarations still include every target.
    // Cross-target resolution would fetch unused platform crates during an offline test.
    let rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("-vV")
        .output()
        .expect("read compiler host");
    assert!(rustc.status.success(), "rustc -vV failed");
    let version = String::from_utf8(rustc.stdout).expect("compiler version");
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("compiler host triple");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--filter-platform",
            host,
        ])
        .current_dir(root)
        .output()
        .expect("run Cargo metadata for the application dependency boundary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Cargo metadata JSON")
}

#[test]
fn application_dependencies_are_only_the_domain_contract() {
    let errors = dependencies::check(&workspace_metadata());
    assert!(
        errors.is_empty(),
        "application dependency boundary:\n{}",
        errors.join("\n")
    );
}

#[path = "application_boundary_support/dependencies.tests.rs"]
mod dependency_tests;
#[path = "application_boundary_support/source.tests.rs"]
mod source_tests;
#[test]
fn production_hosts_exclude_retired_transport_and_experimental_peer() {
    let metadata = workspace_metadata();
    let packages = metadata["packages"].as_array().expect("packages");
    assert!(
        packages
            .iter()
            .all(|package| package["name"] != "magistr-protocol"),
        "the unserved engine-wire contract must not return to production"
    );
    let engine = packages
        .iter()
        .find(|package| package["name"] == "magistr-engine")
        .expect("engine");
    let node = metadata["resolve"]["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == engine["id"])
        .expect("resolved engine");
    assert!(
        node["features"]
            .as_array()
            .expect("engine features")
            .iter()
            .all(|feature| feature != "experimental-peer"),
        "production hosts must not enable the peer experiment"
    );
}
