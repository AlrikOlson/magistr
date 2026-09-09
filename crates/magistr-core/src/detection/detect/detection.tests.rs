use super::*;
use crate::detection::detect::confidence::Confidence;

#[test]
fn detection_new() {
    let d = Detection::new("Rust", Confidence::high(), "Cargo.toml found");
    assert_eq!(d.value, "Rust");
    assert_eq!(d.confidence.score(), 95);
    assert_eq!(d.evidence.len(), 1);
}

#[test]
fn detection_none_has_zero_confidence() {
    let d = Detection::none("Unknown");
    assert_eq!(d.confidence.score(), 0);
    assert!(d.evidence.is_empty());
}

#[test]
fn detection_needs_override_when_low() {
    let low = Detection::new("test", Confidence::low(), "heuristic");
    let high = Detection::new("test", Confidence::high(), "definitive");
    assert!(low.needs_override());
    assert!(!high.needs_override());
}

#[test]
fn detection_display() {
    let d = Detection::new("Rust", Confidence::new(95), "Cargo.toml");
    assert_eq!(format!("{d}"), "Rust (95%)");
}
