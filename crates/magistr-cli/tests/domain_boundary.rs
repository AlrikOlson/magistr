//! The required workspace test gate enforces the domain's effect boundary.
//! AST inspection complements `no_std`: it also sees inactive cfg branches,
//! test sources and macro arguments. Cargo metadata closes dependency aliases,
//! target-specific dependencies and transitive edges that source paths cannot.

#[path = "domain_boundary_support/dependencies.rs"]
mod dependencies;
#[path = "domain_boundary_support/source.rs"]
mod source;

use std::path::Path;
use std::process::Command;

#[test]
fn domain_sources_cannot_access_effects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../magistr-domain");
    let errors = source::check_tree(&root);
    assert!(
        errors.is_empty(),
        "domain source boundary:\n{}",
        errors.join("\n")
    );
}

#[test]
fn domain_dependencies_cannot_access_effects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .current_dir(root)
        .output()
        .expect("run Cargo metadata for the domain dependency boundary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = serde_json::from_slice(&output.stdout).expect("Cargo metadata JSON");
    let errors = dependencies::check(&metadata);
    assert!(
        errors.is_empty(),
        "domain dependency boundary:\n{}",
        errors.join("\n")
    );
}

#[path = "domain_boundary_support/dependencies.tests.rs"]
mod dependency_tests;
#[path = "domain_boundary_support/source.tests.rs"]
mod source_tests;
