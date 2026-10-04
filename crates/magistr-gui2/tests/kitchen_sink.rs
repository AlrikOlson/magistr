//! The sink's completeness gate.
//!
//! rule — a component not in the sink cannot ship in a view — made executable: the set
//! of component modules is derived from `src/` itself, so a new component that skips
//! the sink fails here by name, and a new infrastructure module must be consciously
//! added to the allowlist below.

use std::collections::BTreeSet;
use std::path::Path;

use magistr_gui2::{Medium, Sink};

/// The modules that are the sink's substrate rather than its subject:
/// infrastructure, the runtimes, the design oracles. Everything in
/// `src/` that is not named here must appear in the sink's catalog.
const INFRASTRUCTURE: &[&str] = &[
    // the accessibility layer
    "access",
    "access_node",
    "element_access",
    // the a1 element substrate itself
    "element",
    "element_emit",
    "element_id",
    "element_layout",
    "element_state",
    // rasterization, text and marks machinery
    "atom_still",
    "fonts",
    "glyph",
    "icon",
    "icon_role",
    "image_quad",
    "mark",
    "mark_raster",
    "marks",
    "mask",
    "oklab",
    "shape",
    "sigil",
    "svg_path",
    "text",
    // Shared-span painting and word wrapping are text machinery used by the exhibits.
    "rich_text",
    "type_scale",
    "type_set",
    "ui_faces",
    "display_face",
    "voice",
    "voice_run",
    // the design vocabulary the components read, not draw
    "elevation",
    "lexicon",
    "medium",
    "severity",
    // the copy-law vocabulary: the stamp a component leaves on its
    // runs and the run the sweep reads back (tests/copy_laws.rs)
    "component",
    "text_run",
    // the cockpit-Model frame and its machinery
    "fixture",
    "fixture_roadmap",
    "roadmap_layout",
    "frame",
    "frame_access",
    "frame_clip",
    "frame_workspace",
    "workspace_layout",
    "metrics",
    "sky",
    // the Galaxy's field: a drawing, its layout, its derivation, and the
    // name interner the labels' sigils need
    "galaxy",
    "galaxy_layout",
    "placed_cluster",
    "cluster",
    // the Trace view's branch graph: a drawing and the stop it reads
    "trace_graph",
    "lane_stop",
    // Birth's field drawings: the survey's sweep and the charting's
    // constellation, drawn by the frame into the composition's field
    "radar",
    "chart",
    "seed",
    "interned",
    // the crate root and the runtimes with their seams
    "lib",
    "cadence",
    "error",
    "input",
    "main",
    "native",
    "route",
    "window",
    // measurement
    "parity",
    "region",
    "region_verdict",
    "verdict",
    // the sink itself
    "sink",
    "sink_section",
    "sink_window",
];

/// Every `src/` module, by file listing — the ground truth the roster
/// is measured against.
fn source_modules() -> BTreeSet<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    std::fs::read_dir(&src)
        .expect("src/ lists")
        .filter_map(|entry| {
            let name = entry.expect("a directory entry").file_name();
            let name = name.to_string_lossy();
            let module = name.strip_suffix(".rs")?;
            if module.ends_with(".tests") {
                return None;
            }
            Some(module.to_owned())
        })
        .collect()
}

/// The modules the sink's catalog exhibits, by its own naming.
fn catalog_modules() -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    for section in Sink::new(Medium::Night).catalog() {
        for (named, _) in section.entries {
            modules.extend(named.iter().map(|&module| module.to_owned()));
        }
    }
    modules
}

/// Every component module is in the sink, by name — the acceptance's
/// executable form. A failure names exactly the module that skipped it.
#[test]
fn every_component_module_is_in_the_sink() {
    let sources = source_modules();
    let catalog = catalog_modules();
    let missing: Vec<&String> = sources
        .iter()
        .filter(|module| !INFRASTRUCTURE.contains(&module.as_str()))
        .filter(|module| !catalog.contains(*module))
        .collect();
    assert!(
        missing.is_empty(),
        "components not in the kitchen sink (add an exhibit to Sink::catalog, \
         or — for infrastructure only — allowlist it in kitchen_sink.rs): {missing:?}"
    );
}

/// The catalog and the allowlist both speak only of modules that exist
/// — a rename cannot leave a stale name quietly satisfying the gate.
#[test]
fn the_roster_names_no_ghost_modules() {
    let sources = source_modules();
    for module in catalog_modules() {
        assert!(
            sources.contains(&module),
            "the catalog names {module}, which is not a module in src/"
        );
    }
    for module in INFRASTRUCTURE {
        assert!(
            sources.contains(*module),
            "the allowlist names {module}, which is not a module in src/"
        );
    }
    let overlap: Vec<&str> = INFRASTRUCTURE
        .iter()
        .copied()
        .filter(|module| catalog_modules().contains(*module))
        .collect();
    assert!(
        overlap.is_empty(),
        "allowlisted as infrastructure yet exhibited in the catalog — drop it \
         from the allowlist: {overlap:?}"
    );
}
