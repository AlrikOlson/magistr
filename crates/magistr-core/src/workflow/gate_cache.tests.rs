use super::*;
use std::path::Path;
use std::time::Duration;

use crate::validate::{Gate, GateResult, ValidationResult};

// ── GateCache tests ─────────────────────────────────────────────────

#[test]
fn cache_new_is_empty() {
    let cache = GateCache::new();
    assert!(cache.source_hash.is_none());
    assert!(!cache.has_results());
}

#[test]
fn cache_default_is_empty() {
    let cache = GateCache::default();
    assert!(!cache.has_results());
}

#[test]
fn cache_invalid_when_empty() {
    let dir = tempfile::TempDir::new().unwrap();
    let cache = GateCache::new();
    assert!(!cache.is_valid(dir.path()));
}

#[test]
fn cache_valid_after_store() {
    let dir = tempfile::TempDir::new().unwrap();
    // Create a source file so hash is non-trivial
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let mut cache = GateCache::new();
    let result = make_validation_result(true, true, true);
    cache.store(dir.path(), result);

    assert!(cache.is_valid(dir.path()));
    assert!(cache.has_results());
    assert!(cache.get(dir.path()).is_some());
}

#[test]
fn cache_invalid_after_file_change() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let mut cache = GateCache::new();
    cache.store(dir.path(), make_validation_result(true, true, true));
    assert!(cache.is_valid(dir.path()));

    // Modify the file — cache should be invalid
    // Sleep briefly to ensure mtime changes
    std::thread::sleep(Duration::from_millis(10));
    std::fs::write(dir.path().join("main.rs"), "fn main() { todo!() }").unwrap();

    assert!(!cache.is_valid(dir.path()));
    assert!(cache.get(dir.path()).is_none());
}

#[test]
fn cache_invalid_after_new_file() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let mut cache = GateCache::new();
    cache.store(dir.path(), make_validation_result(true, true, true));
    assert!(cache.is_valid(dir.path()));

    // Add a new source file
    std::fs::write(dir.path().join("lib.rs"), "pub fn foo() {}").unwrap();

    assert!(!cache.is_valid(dir.path()));
}

#[test]
fn cache_invalidate_clears() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let mut cache = GateCache::new();
    cache.store(dir.path(), make_validation_result(true, true, true));
    assert!(cache.has_results());

    cache.invalidate();
    assert!(!cache.has_results());
    assert!(!cache.is_valid(dir.path()));
}

#[test]
fn cache_ignores_non_source_files() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let mut cache = GateCache::new();
    cache.store(dir.path(), make_validation_result(true, true, true));
    assert!(cache.is_valid(dir.path()));

    // Add a non-source file — cache should remain valid
    std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();
    std::fs::write(dir.path().join("image.png"), [0u8; 100]).unwrap();

    assert!(cache.is_valid(dir.path()));
}

#[test]
fn cache_skips_hidden_and_build_dirs() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let mut cache = GateCache::new();
    cache.store(dir.path(), make_validation_result(true, true, true));

    // Add files in directories that should be skipped
    let target_dir = dir.path().join("target/debug");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(target_dir.join("build.rs"), "compiled").unwrap();

    let hidden_dir = dir.path().join(".git/objects");
    std::fs::create_dir_all(&hidden_dir).unwrap();
    std::fs::write(hidden_dir.join("data.rs"), "git internal").unwrap();

    let node_dir = dir.path().join("node_modules/pkg");
    std::fs::create_dir_all(&node_dir).unwrap();
    std::fs::write(node_dir.join("index.js"), "module.exports").unwrap();

    assert!(cache.is_valid(dir.path()));
}

// ── Source extension tests ───────────────────────────────────────────

#[test]
fn source_extension_detection() {
    assert!(is_source_extension(Path::new("main.rs")));
    assert!(is_source_extension(Path::new("app.ts")));
    assert!(is_source_extension(Path::new("index.tsx")));
    assert!(is_source_extension(Path::new("lib.py")));
    assert!(is_source_extension(Path::new("main.go")));
    assert!(is_source_extension(Path::new("Cargo.toml")));
    assert!(is_source_extension(Path::new("config.json")));
    assert!(is_source_extension(Path::new("App.java")));

    assert!(!is_source_extension(Path::new("README.md")));
    assert!(!is_source_extension(Path::new("image.png")));
    assert!(!is_source_extension(Path::new("data.csv")));
    assert!(!is_source_extension(Path::new("no_extension")));
}

// ── Helpers ─────────────────────────────────────────────────────────

fn make_validation_result(check: bool, test: bool, lint: bool) -> ValidationResult {
    ValidationResult {
        results: vec![
            GateResult {
                gate: Gate::Check,
                passed: check,
                duration: Duration::from_secs(1),
                output: if check {
                    "ok".into()
                } else {
                    "check failed".into()
                },
                command: "cargo check".into(),
            },
            GateResult {
                gate: Gate::Test,
                passed: test,
                duration: Duration::from_secs(3),
                output: if test {
                    "ok".into()
                } else {
                    "test failed".into()
                },
                command: "cargo test".into(),
            },
            GateResult {
                gate: Gate::Lint,
                passed: lint,
                duration: Duration::from_secs(2),
                output: if lint {
                    "ok".into()
                } else {
                    "lint failed".into()
                },
                command: "cargo clippy".into(),
            },
        ],
    }
}
