//! The desktop's hygiene gates, run for real against temp trees:
//! `scripts/lint-structure.sh` rule 5 refuses an `allow(dead_code)` /
//! `allow(unused)` under `crates/magistr-desktop`, and
//! `scripts/lint-duplication.sh` refuses a file copied from the owner tree
//! (byte-identical or a ≥50-token clone) while ignoring the owner tree's own
//! internal duplication.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn script(name: &str) -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("scripts")
        .join(name)
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

/// The script the way the gate runs it: through `sh -c`, which follows
/// its shebang to bash on every platform (on Windows, Git's).
fn run(name: &str, root: &Path) -> Output {
    let script = script(name).to_string_lossy().replace('\\', "/");
    Command::new("sh")
        .arg("-c")
        .arg(format!("'{script}'"))
        .env("LINT_PROJECT_ROOT", root)
        .output()
        .unwrap_or_else(|e| panic!("run {name}: {e}"))
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

const CLEAN_LIB: &str = "#![deny(dead_code, unused)]\npub fn live() -> u8 { 1 }\n";

/// A owner-looking module long enough to clear jscpd's 50-token bar.
fn owner_module(name: &str) -> String {
    format!(
        "pub struct {name} {{ pub width: u16, pub height: u16, pub cells: Vec<u8> }}\n\
         impl {name} {{\n\
         \x20   pub fn new(width: u16, height: u16) -> Self {{\n\
         \x20       let cells = vec![0u8; usize::from(width) * usize::from(height)];\n\
         \x20       Self {{ width, height, cells }}\n\
         \x20   }}\n\
         \x20   pub fn index(&self, col: u16, row: u16) -> Option<usize> {{\n\
         \x20       (col < self.width && row < self.height)\n\
         \x20           .then(|| usize::from(row) * usize::from(self.width) + usize::from(col))\n\
         \x20   }}\n\
         \x20   pub fn get(&self, col: u16, row: u16) -> Option<u8> {{\n\
         \x20       self.index(col, row).map(|i| self.cells[i])\n\
         \x20   }}\n\
         }}\n"
    )
}

// --- lint-structure rule 5 ---------------------------------------------------

#[test]
fn structure_lint_accepts_a_clean_desktop_crate() {
    let tmp = TempDir::new().expect("tempdir");
    write(tmp.path(), "crates/magistr-desktop/src/lib.rs", CLEAN_LIB);
    let out = run("lint-structure.sh", tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn structure_lint_refuses_an_allow_dead_code_under_the_desktop_crate() {
    let tmp = TempDir::new().expect("tempdir");
    write(tmp.path(), "crates/magistr-desktop/src/lib.rs", CLEAN_LIB);
    write(
        tmp.path(),
        "crates/magistr-desktop/src/render/stale.rs",
        "#[allow(dead_code)]\nfn nobody_calls_me() {}\n",
    );
    let out = run("lint-structure.sh", tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains("crates/magistr-desktop/src/render/stale.rs") && log.contains("dead_code"),
        "{log}"
    );
}

#[test]
fn structure_lint_refuses_allow_unused_and_expect_forms_too() {
    for attr in [
        "#![allow(unused)]",
        "#[allow(clippy::all, unused_imports)]",
        "#[expect(dead_code)]",
    ] {
        let tmp = TempDir::new().expect("tempdir");
        write(
            tmp.path(),
            "crates/magistr-desktop/src/lib.rs",
            &format!("{attr}\npub fn live() -> u8 {{ 1 }}\n"),
        );
        let out = run("lint-structure.sh", tmp.path());
        assert!(!out.status.success(), "{attr} passed:\n{}", text(&out));
    }
}

#[test]
fn structure_lint_ignores_the_words_outside_an_attribute() {
    // A test that greps for the attribute must not itself trip the rule.
    let tmp = TempDir::new().expect("tempdir");
    write(tmp.path(), "crates/magistr-desktop/src/lib.rs", CLEAN_LIB);
    write(
        tmp.path(),
        "crates/magistr-desktop/tests/boundary.rs",
        "const NEEDLES: [&str; 2] = [\"allow(dead_code\", \"allow(unused\"];\n",
    );
    let out = run("lint-structure.sh", tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

// --- lint-duplication -------------------------------------------------------

#[test]
fn duplication_gate_accepts_distinct_trees() {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-cli/src/application/grid.rs",
        &owner_module("Grid"),
    );
    write(
        tmp.path(),
        "crates/magistr-desktop/src/lib.rs",
        "pub fn width(text: &str) -> u16 { text.chars().count() as u16 }\n",
    );
    let out = run("lint-duplication.sh", tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn duplication_gate_refuses_a_byte_identical_copy() {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-cli/src/application/grid.rs",
        &owner_module("Grid"),
    );
    write(
        tmp.path(),
        "crates/magistr-desktop/src/buffer.rs",
        &owner_module("Grid"),
    );
    let out = run("lint-duplication.sh", tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains("byte-identical")
            && log.contains("crates/magistr-cli/src/application/grid.rs"),
        "{log}"
    );
}

#[test]
fn duplication_gate_refuses_a_renamed_clone() {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-cli/src/application/grid.rs",
        &owner_module("Grid"),
    );
    // Same code, different type name and an extra function: not identical,
    // but a clone well over 50 tokens.
    let mut clone = owner_module("Buffer");
    clone.push_str("pub fn extra() -> u8 { 7 }\n");
    write(tmp.path(), "crates/magistr-desktop/src/buffer.rs", &clone);
    let out = run("lint-duplication.sh", tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains("clone") && log.contains("crates/magistr-desktop/src/buffer.rs"),
        "{log}"
    );
}

#[test]
fn duplication_gate_ignores_the_owner_trees_internal_duplication() {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-cli/src/application/grid.rs",
        &owner_module("Grid"),
    );
    write(
        tmp.path(),
        "crates/magistr-cli/src/application/grid_copy.rs",
        &owner_module("Grid"),
    );
    write(
        tmp.path(),
        "crates/magistr-desktop/src/lib.rs",
        "pub fn width(text: &str) -> u16 { text.chars().count() as u16 }\n",
    );
    let out = run("lint-duplication.sh", tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn duplication_gate_is_wired_as_a_required_custom_gate() {
    let config = std::fs::read_to_string(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join(".magistr/project.json"),
    )
    .expect("read project.json");
    let json: serde_json::Value = serde_json::from_str(&config).expect("parse project.json");
    let gates = json["custom_gates"].as_array().expect("custom_gates");
    for (name, command) in [
        ("duplication", "./scripts/lint-duplication.sh"),
        ("machete", "./scripts/machete-gate.sh"),
    ] {
        assert!(
            gates
                .iter()
                .any(|g| g["name"] == name && g["command"] == command && g["required"] == true),
            "{name} is not a required custom gate: {gates:?}"
        );
    }
}
