//! Verify that the element tree uses pixel layout without terminal grid, layout,
//! rendering, or text-role imports. Cockpit `Msg` remains its shared event vocabulary.

use std::path::Path;

/// The substrate's modules — the sources the boundary holds over.
const MODULES: &[&str] = &[
    "src/element.rs",
    "src/element_id.rs",
    "src/element_layout.rs",
    "src/element_state.rs",
    "src/element_emit.rs",
    "src/element_access.rs",
];

/// What must never appear, and why it is banned.
const BANNED: &[(&str, &str)] = &[
    (
        "cell",
        "the grid unit, in any casing — the substrate is pixels end to end",
    ),
    ("Cell", "the grid unit, in any casing"),
    ("CELL", "the grid unit, in any casing"),
    (
        "TextRole",
        "the cockpit's role mapping — the substrate speaks Voice",
    ),
    (
        "magistr_cockpit::layout",
        "the element tree must use native pixel layout",
    ),
    (
        "magistr_cockpit::render",
        "the cockpit's render vocabulary rides the same tree",
    ),
    ("Basis", "the px-per-unit seam only the shared tree needs"),
];

#[test]
fn the_substrate_never_speaks_the_terminals_units() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offences = Vec::new();
    for module in MODULES {
        let source = std::fs::read_to_string(root.join(module))
            .unwrap_or_else(|error| panic!("{module} must exist and read: {error}"));
        for (token, why) in BANNED {
            for (number, line) in source.lines().enumerate() {
                if line.contains(token) {
                    offences.push(format!(
                        "{module}:{}: `{token}` ({why})\n    {}",
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the substrate's boundary is breached:\n{}",
        offences.join("\n")
    );
}

/// The flip side: the substrate really is on the shared loop — its
/// state module folds the cockpit's own `Msg`, not a private event
/// type. A second event system failing to exist is as load-bearing as
/// the units being pixels.
#[test]
fn the_state_module_folds_the_cockpits_own_messages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("src/element_state.rs"))
        .expect("the state module exists");
    assert!(
        source.contains("use magistr_cockpit::cockpit::Msg;"),
        "element_state must import the shared Msg"
    );
    assert!(
        source.contains("msg: &Msg"),
        "and fold it, rather than invent its own event type"
    );
}
