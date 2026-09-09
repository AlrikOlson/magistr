use super::*;

use std::io::Write;
use tempfile::TempDir;

use crate::types::Language;

#[test]
fn fs_detector_detects_rust_project() {
    let tmp = TempDir::new().unwrap();
    let cargo_toml = tmp.path().join("Cargo.toml");
    let mut f = std::fs::File::create(&cargo_toml).unwrap();
    writeln!(
        f,
        r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021""#
    )
    .unwrap();

    let detector = FileSystemDetector;
    let stack = detector.detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.project_name, "test-crate");
}

#[test]
fn fs_detector_implements_detector_trait() {
    // Verify FileSystemDetector can be used as dyn Detector
    let detector: Box<dyn Detector> = Box::new(FileSystemDetector);
    let tmp = TempDir::new().unwrap();
    let cargo_toml = tmp.path().join("Cargo.toml");
    std::fs::write(
        &cargo_toml,
        r#"[package]
name = "trait-test"
version = "0.1.0"
edition = "2021""#,
    )
    .unwrap();

    let stack = detector.detect_stack(tmp.path()).unwrap();
    assert_eq!(stack.language, Language::Rust);
}
