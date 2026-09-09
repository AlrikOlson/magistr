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

#[test]
fn application_dependencies_are_only_the_domain_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .current_dir(root)
        .output()
        .expect("run Cargo metadata for the application dependency boundary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = serde_json::from_slice(&output.stdout).expect("Cargo metadata JSON");
    let errors = dependencies::check(&metadata);
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
