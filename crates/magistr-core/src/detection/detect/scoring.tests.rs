use super::*;
use crate::types::Language;
use tempfile::TempDir;

fn empty_manifest() -> ManifestData {
    ManifestData::default()
}

#[test]
fn scoring_context_construction() {
    let tmp = TempDir::new().unwrap();
    let manifest = empty_manifest();
    let ctx = ScoringContext {
        dir: tmp.path(),
        language: Language::Rust,
        manifest: &manifest,
    };
    assert_eq!(ctx.language, Language::Rust);
}

#[test]
fn language_scorable_rust() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
    let manifest = empty_manifest();
    let ctx = ScoringContext {
        dir: tmp.path(),
        language: Language::Rust,
        manifest: &manifest,
    };
    let detection = Language::Rust.score(&ctx);
    assert_eq!(detection.value, Language::Rust);
    assert!(detection.confidence.score() >= 90);
}

#[test]
fn language_scorable_unknown() {
    let tmp = TempDir::new().unwrap();
    let manifest = empty_manifest();
    let ctx = ScoringContext {
        dir: tmp.path(),
        language: Language::Unknown,
        manifest: &manifest,
    };
    let detection = Language::Unknown.score(&ctx);
    assert_eq!(detection.confidence.score(), 0);
}
