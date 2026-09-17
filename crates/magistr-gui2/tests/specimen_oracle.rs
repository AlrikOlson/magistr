//! The specimen oracle's bookkeeping.
//!
//! `scripts/capture-almagest-specimens.mjs` cuts every atom of the blessed volume
//! (`design/almagest-seventh-edition.html`) at its own pixel size, night and day, into
//! `tests/mocks/specimens/` beside a manifest naming each cut: id, figure, selector,
//! pad, media, size. The component layer measures itself against these captures the way
//! view parity measures against `tests/mocks/*.png`.
//!
//! These tests hold the ledger honest: every specimen the manifest names has its
//! capture at the manifest's size, every capture in the directory is named by the
//! manifest, and the classes the design requires cannot fall out of the manifest
//! without this file changing. A missing capture fails BY NAME — never a silent pass
//! over an empty directory.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn specimens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/specimens")
}

fn manifest() -> Vec<serde_json::Value> {
    let path = specimens_dir().join("manifest.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "no specimen manifest at {} ({e}) — run scripts/capture-almagest-specimens.mjs",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("manifest.json parses as an array of specimens")
}

/// The media a specimen's `media` field expands to.
fn media_of(entry: &serde_json::Value) -> Vec<&'static str> {
    match entry["media"].as_str().expect("specimen has a media field") {
        "both" => vec!["night", "day"],
        "night" => vec!["night"],
        "day" => vec!["day"],
        other => panic!("specimen {}: unknown media {other:?}", entry["id"]),
    }
}

/// Width and height out of a PNG's IHDR chunk — the first chunk after
/// the 8-byte signature, its data starting at byte 16.
fn png_size(path: &Path) -> (u64, u64) {
    let bytes = fs::read(path).expect("capture is readable");
    assert!(
        bytes.len() >= 24 && bytes[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'],
        "{} is not a PNG",
        path.display()
    );
    let be = |at: usize| {
        u64::from(u32::from_be_bytes([
            bytes[at],
            bytes[at + 1],
            bytes[at + 2],
            bytes[at + 3],
        ]))
    };
    (be(16), be(20))
}

/// Every specimen the manifest names has its capture on disk, at the
/// manifest's own pixel size, in every medium it declares — and the
/// directory holds no capture the manifest does not name.
#[test]
fn every_specimen_in_the_manifest_has_its_capture_at_its_size() {
    let dir = specimens_dir();
    let mut named = BTreeSet::new();
    let mut wrong = Vec::new();
    for entry in manifest() {
        let id = entry["id"].as_str().expect("specimen has an id");
        let (w, h) = (entry["width"].as_u64(), entry["height"].as_u64());
        for medium in media_of(&entry) {
            let name = format!("{id}-{medium}.png");
            let path = dir.join(&name);
            named.insert(name.clone());
            if !path.is_file() {
                wrong.push(format!("{name}: capture MISSING"));
                continue;
            }
            let actual = png_size(&path);
            if Some(actual.0) != w || Some(actual.1) != h {
                wrong.push(format!(
                    "{name}: capture is {}x{}, manifest says {}x{}",
                    actual.0,
                    actual.1,
                    w.unwrap_or(0),
                    h.unwrap_or(0)
                ));
            }
        }
    }
    for file in fs::read_dir(&dir)
        .expect("specimens directory exists")
        .flatten()
    {
        let name = file.file_name().to_string_lossy().into_owned();
        if name.ends_with(".png") && !named.contains(&name) {
            wrong.push(format!("{name}: capture the manifest does not name"));
        }
    }
    assert!(
        wrong.is_empty(),
        "the specimen oracle and its captures disagree:\n  {}",
        wrong.join("\n  ")
    );
}

/// The classes the design requires cannot quietly leave the manifest:
/// dropping one is a decision made here, not drift in a capture script.
#[test]
fn the_manifest_covers_the_required_specimen_classes() {
    // control classes · card/row/bar/pane surfaces · the meter · severity grounds+caps
    // · the design elevation levels (canvas = pane, row = status row, raised = plan
    // card, overlay, popover) · the design star states.
    const REQUIRED: &[&str] = &[
        "control-data-chip-live",
        "control-nav-chip",
        "control-nav-crumb",
        "control-action-primary",
        "control-action-secondary",
        "control-action-negative",
        "control-segment",
        "control-switch-on",
        "control-switch-off",
        "control-theme-card",
        "control-theme-card-chosen",
        "control-slot",
        "card-plan",
        "row-status-executing",
        "bar-budget",
        "pane-sky",
        "pane-chart",
        "meter",
        "card-ground-attention",
        "card-ground-danger",
        "cap-severity-attention",
        "cap-severity-danger",
        "cap-severity-positive",
        "ground-severity-danger",
        "ground-severity-attention",
        "ground-severity-positive",
        "elevation-overlay",
        "elevation-popover",
        "star-pending",
        "star-ready",
        "star-executing",
        "star-stuck",
        "star-blocked",
        "star-done",
    ];
    let ids: BTreeSet<String> = manifest()
        .iter()
        .map(|e| e["id"].as_str().expect("specimen has an id").to_owned())
        .collect();
    let missing: Vec<&&str> = REQUIRED.iter().filter(|id| !ids.contains(**id)).collect();
    assert!(
        missing.is_empty(),
        "required specimen classes missing from the manifest: {missing:?}"
    );
}
