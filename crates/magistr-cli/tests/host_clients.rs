//! Host entry points open concrete clients before taking over a surface.

use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The entry point observes the engines before it takes the terminal over:
/// a store that cannot be opened is reported on a normal stdout, never
/// inside the alternate screen. A viewer observes rather than admits itself
/// as the project's host, so it never contends with a running loop.
#[test]
fn the_example_observes_the_engines_before_the_terminal() {
    let main = crate_root().join("examples/cockpit/main.rs");
    let source = std::fs::read_to_string(main).expect("examples/cockpit/main.rs");
    assert!(
        !source.contains("EngineClient::open("),
        "a viewer must not take the host lease"
    );
    let engines = source
        .find("EngineClient::observe(")
        .expect("the example observes the engines");
    let terminal = source
        .find("RealTerminal::stdio()")
        .expect("the example takes the terminal over");
    assert!(engines < terminal, "the engines open before raw mode");
    let report = source[engines..terminal]
        .find("println!")
        .expect("a failure to open is printed before the terminal is touched");
    assert!(engines < engines + report);
}
