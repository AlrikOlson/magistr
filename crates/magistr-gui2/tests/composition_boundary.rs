//! The composition boundary.
//!
//! A composition is component instances and layout only — it maps the shared `Model` to
//! props and builds an `Element` tree. It never draws: no `DrawList` emission, no
//! painter, no rasterizer, and no reach back into the cockpit's cell `Node` tree. This
//! probe makes the acceptance criterion executable over the sources themselves.

use std::fs;
use std::path::PathBuf;

/// What must never appear in a `comp_*` module, and why it is banned.
const BANNED: &[(&str, &str)] = &[
    (
        "DrawList",
        "a composition maps to components; it emits nothing",
    ),
    ("emit(", "emission is element_emit's alone"),
    ("Painter", "the painters draw; a composition arranges"),
    ("CpuRasterizer", "rasterization is the still's business"),
    ("cpu_raster", "rasterization is the still's business"),
    ("qs_gpu::frame", "the frame vocabulary is the renderer's"),
    ("Pixmap", "a composition holds no pixels"),
    (
        "Node::",
        "the cockpit's cell tree is the terminal's; compositions build Elements",
    ),
];

fn composition_sources() -> Vec<(String, String)> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    for entry in fs::read_dir(&src).expect("src/ lists") {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if name.starts_with("comp_") && name.ends_with(".rs") {
            let text = fs::read_to_string(&path).expect("a composition source reads");
            sources.push((name, text));
        }
    }
    assert!(
        sources.len() >= 7,
        "the seven composition modules (and their tests) are probed; found {}",
        sources.len()
    );
    sources
}

#[test]
fn a_composition_is_components_and_layout_only() {
    for (name, text) in composition_sources() {
        for (token, why) in BANNED {
            assert!(!text.contains(token), "{name} names `{token}` — {why}");
        }
    }
}

/// A composition holds no state of its own: every `comp_*` module's one
/// public item is a unit struct, so what shows is always a fresh read
/// of the model.
#[test]
fn a_composition_holds_no_state() {
    for (name, text) in composition_sources() {
        if name.ends_with(".tests.rs") {
            continue;
        }
        let stateless = text.contains("Composition;");
        assert!(
            stateless,
            "{name}'s composition is a unit struct — fields would be state the model already holds"
        );
        assert!(
            !text.contains("&mut self") && !text.contains("mut self"),
            "{name} mutates nothing — a composition is a pure read"
        );
    }
}
