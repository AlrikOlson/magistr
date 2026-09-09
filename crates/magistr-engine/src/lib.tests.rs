/// Verify that magistr-engine has zero TUI dependencies.
///
/// Parses engine/Cargo.toml and asserts that ratatui, crossterm, and
/// tachyonfx do not appear in [dependencies]. This prevents accidental
/// coupling between the domain model and the terminal UI layer.
#[test]
fn engine_has_no_tui_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let forbidden = ["ratatui", "crossterm", "tachyonfx"];

    // Parse only the [dependencies] section (skip [dev-dependencies], comments, etc.)
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps {
            for dep in &forbidden {
                assert!(
                    !trimmed.starts_with(dep),
                    "magistr-engine must not depend on TUI crate `{dep}` — found in Cargo.toml: {trimmed}"
                );
            }
        }
    }
}
