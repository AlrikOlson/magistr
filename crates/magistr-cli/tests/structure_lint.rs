//! The cohesive-module rule of `scripts/lint-structure.sh`, run for real
//! against temp trees. A file may export several public items when it is a
//! private module re-exported through its parent facade, opens with a `//!`
//! doc naming what those items decide together, and has its companion
//! `{name}.tests.rs`; each missing property is refused by name. Single-item
//! files and grandfathered files keep passing.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn script() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("scripts")
        .join("lint-structure.sh")
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

/// The script the way the gate runs it: through `sh -c`, which follows its
/// shebang to bash.
fn run(root: &Path) -> Output {
    let script = script().to_string_lossy().replace('\\', "/");
    Command::new("sh")
        .arg("-c")
        .arg(format!("'{script}'"))
        .env("LINT_PROJECT_ROOT", root)
        .output()
        .unwrap_or_else(|e| panic!("run lint-structure.sh: {e}"))
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

const MODULE: &str = "crates/magistr-domain/src/policy/permission.rs";
const TESTS: &str = "crates/magistr-domain/src/policy/permission.tests.rs";
const FACADE: &str = "crates/magistr-domain/src/policy/mod.rs";

const RATIONALE: &str = "//! One permission decision: the context a host observes, the outcome and\n\
                         //! its source; a new source is produced and carried here together.\n";
const ITEMS: &str = "pub struct PermissionContext { pub protected: bool }\n\
                     pub enum PermissionOutcome { Allow, Deny }\n\
                     #[cfg(test)]\n#[path = \"permission.tests.rs\"]\nmod tests;\n";
const PRIVATE_FACADE: &str =
    "mod permission;\npub use permission::{PermissionContext, PermissionOutcome};\n";

/// The admitted shape: private module, facade re-export, rationale, tests.
fn cohesive_tree() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-domain/src/lib.rs",
        "pub mod policy;\n",
    );
    write(tmp.path(), FACADE, PRIVATE_FACADE);
    write(tmp.path(), MODULE, &format!("{RATIONALE}{ITEMS}"));
    write(tmp.path(), TESTS, "use super::*;\n");
    tmp
}

#[test]
fn a_cohesive_module_behind_a_private_facade_with_rationale_and_tests_passes() {
    let tmp = cohesive_tree();
    let out = run(tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn a_public_module_path_may_still_export_only_one_item() {
    let tmp = cohesive_tree();
    write(
        tmp.path(),
        FACADE,
        "pub mod permission;\npub use permission::{PermissionContext, PermissionOutcome};\n",
    );
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains(MODULE) && log.contains("pub mod"),
        "the refusal must name the file and the public module path:\n{log}"
    );
}

#[test]
fn a_private_module_nobody_re_exports_is_refused() {
    let tmp = cohesive_tree();
    write(tmp.path(), FACADE, "mod permission;\n");
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains(MODULE) && log.contains("pub use permission::"),
        "the refusal must name the missing re-export:\n{log}"
    );
}

#[test]
fn a_module_no_parent_declares_is_refused() {
    let tmp = cohesive_tree();
    write(
        tmp.path(),
        FACADE,
        "pub use permission::PermissionContext;\n",
    );
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains(MODULE) && log.contains("mod permission;"),
        "the refusal must name the missing declaration:\n{log}"
    );
}

#[test]
fn a_cohesive_module_without_a_change_rationale_is_refused() {
    let tmp = cohesive_tree();
    write(tmp.path(), MODULE, ITEMS);
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains(MODULE) && log.contains("//!"),
        "the refusal must name the missing module doc:\n{log}"
    );
}

#[test]
fn a_cohesive_module_without_companion_tests_is_refused() {
    let tmp = cohesive_tree();
    std::fs::remove_file(tmp.path().join(TESTS)).expect("remove tests");
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains(MODULE) && log.contains(TESTS),
        "the refusal must name the missing companion tests:\n{log}"
    );
}

#[test]
fn every_missing_property_is_named_at_once() {
    let tmp = cohesive_tree();
    write(tmp.path(), FACADE, "pub mod permission;\n");
    write(tmp.path(), MODULE, ITEMS);
    std::fs::remove_file(tmp.path().join(TESTS)).expect("remove tests");
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    for needle in ["pub mod", "//!", TESTS] {
        assert!(log.contains(needle), "missing {needle:?} in:\n{log}");
    }
}

#[test]
fn single_item_files_need_no_facade_doc_or_tests() {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-domain/src/lib.rs",
        "pub mod policy;\n",
    );
    write(
        tmp.path(),
        FACADE,
        "pub mod permission_mode;\npub use permission_mode::PermissionMode;\n",
    );
    write(
        tmp.path(),
        "crates/magistr-domain/src/policy/permission_mode.rs",
        "pub enum PermissionMode { Standard, Workflow }\n",
    );
    let out = run(tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn the_facade_may_be_a_sibling_file_or_the_crate_root() {
    let tmp = TempDir::new().expect("tempdir");
    // A `policy.rs` sibling declares `policy/permission.rs`.
    write(
        tmp.path(),
        "crates/magistr-engine/src/lib.rs",
        "pub mod policy;\nmod scope;\npub use scope::{ScopeA, ScopeB};\n",
    );
    write(
        tmp.path(),
        "crates/magistr-engine/src/policy.rs",
        PRIVATE_FACADE,
    );
    write(
        tmp.path(),
        "crates/magistr-engine/src/policy/permission.rs",
        &format!("{RATIONALE}{ITEMS}"),
    );
    write(
        tmp.path(),
        "crates/magistr-engine/src/policy/permission.tests.rs",
        "use super::*;\n",
    );
    // The crate root declares `scope.rs`.
    write(
        tmp.path(),
        "crates/magistr-engine/src/scope.rs",
        "//! Two scopes that are always widened together.\npub struct ScopeA;\npub struct ScopeB;\n",
    );
    write(
        tmp.path(),
        "crates/magistr-engine/src/scope.tests.rs",
        "use super::*;\n",
    );
    let out = run(tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn a_crate_root_cannot_be_a_cohesive_module() {
    let tmp = TempDir::new().expect("tempdir");
    write(
        tmp.path(),
        "crates/magistr-application/src/lib.rs",
        "//! Root.\npub struct A;\npub struct B;\n",
    );
    write(
        tmp.path(),
        "crates/magistr-application/src/lib.tests.rs",
        "use super::*;\n",
    );
    let out = run(tmp.path());
    let log = text(&out);
    assert!(!out.status.success(), "gate stayed green:\n{log}");
    assert!(
        log.contains("crates/magistr-application/src/lib.rs"),
        "{log}"
    );
}

#[test]
fn a_grandfathered_file_stays_admitted_without_the_properties() {
    let tmp = cohesive_tree();
    write(tmp.path(), FACADE, "pub mod permission;\n");
    write(tmp.path(), MODULE, ITEMS);
    std::fs::remove_file(tmp.path().join(TESTS)).expect("remove tests");
    write(
        tmp.path(),
        ".magistr/lint-structure-allowlist.txt",
        &format!("{MODULE}\n"),
    );
    let out = run(tmp.path());
    assert!(out.status.success(), "{}", text(&out));
}

#[test]
fn the_allowlist_names_only_files_that_exist() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let allowlist = std::fs::read_to_string(root.join(".magistr/lint-structure-allowlist.txt"))
        .expect("read allowlist");
    let stale: Vec<&str> = allowlist
        .lines()
        .filter(|line| !line.is_empty() && !root.join(line).is_file())
        .collect();
    assert!(stale.is_empty(), "stale allowlist entries: {stale:?}");
}
