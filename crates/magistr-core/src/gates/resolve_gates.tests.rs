use super::*;
#[test]
fn clippy_consolidates_only_the_same_cargo_coverage() {
    assert!(clippy_covers(
        "cargo check --workspace --all-targets",
        "cargo clippy --workspace --all-targets -- -D warnings"
    ));
    for (check, lint) in [
        ("cargo check --all-targets", "cargo clippy"),
        (
            "cargo check --features desktop",
            "cargo clippy --features terminal",
        ),
        ("cargo check -p one", "cargo clippy -p two"),
        ("cd one && cargo check", "cd two && cargo clippy"),
        ("cargo check --release", "cargo clippy"),
        ("cargo check; echo ok", "cargo clippy; echo ok"),
    ] {
        assert!(!clippy_covers(check, lint), "{check} vs {lint}");
    }
}
