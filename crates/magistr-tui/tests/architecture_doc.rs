//! The architecture doc names what is actually here:
//! `docs/reference/cockpit-modules.md` carries a row per module of
//! magistr-cockpit and of magistr-tui, and this test is what stops that
//! index from becoming a list of files that used to exist.
//!
//! Both directions matter. A module with no row is an undocumented
//! module — the failure mode a hand-written map always ends in. A row
//! naming a file that is gone is worse: it reads as current.
//!
//! The same rule covers the integration tests of both crates, because
//! the index's third column names them: a harness added under `tests/`
//! and never mentioned leaves modules pointing at the wrong guard.
//!
//! Rows are spelled from `crates/` — `magistr-cockpit/src/…` and
//! `magistr-tui/src/…` — so a reader knows which side of the seam a
//! module is on without opening it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The two crates the doc describes, as the doc spells them and where
/// they are relative to this crate's manifest.
const CRATES: [(&str, &str); 2] = [
    ("magistr-cockpit", "../magistr-cockpit"),
    ("magistr-tui", "."),
];

/// The doc, read from the repository root above this crate.
fn doc() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reference/cockpit-modules.md")
        .canonicalize()
        .expect("docs/reference/cockpit-modules.md exists");
    std::fs::read_to_string(path).expect("read the architecture doc")
}

fn crate_dir(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Every `.rs` file of a crate that is not a companion test, as
/// `<crate>/src/<path>`.
fn modules(name: &str, relative: &str) -> BTreeSet<String> {
    let src = crate_dir(relative).join("src");
    let mut out = BTreeSet::new();
    walk(&src, &src, &mut out);
    out.into_iter()
        .map(|rel| format!("{name}/src/{rel}"))
        .collect()
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    let entries = std::fs::read_dir(dir).expect("read a source directory");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.to_string_lossy().ends_with(".tests.rs")
        {
            let rel = path.strip_prefix(root).expect("under src");
            out.insert(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// The `.rs` paths the doc names in backticks that start with `prefix`.
/// A row's first column is the module; anything else backticked that
/// ends in `.rs` — a test file named in prose — is deliberately included,
/// since the point is that every name the doc prints is a name that
/// exists.
fn named(doc: &str, prefix: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for token in doc.split('`').skip(1).step_by(2) {
        if token.starts_with(prefix) && token.ends_with(".rs") && !token.contains(' ') {
            out.insert(token.to_string());
        }
    }
    out
}

#[test]
fn the_index_names_every_module_of_both_crates_and_no_others() {
    let doc = doc();
    let mut modules_seen = BTreeSet::new();
    let mut listed = BTreeSet::new();
    for (name, relative) in CRATES {
        let found = modules(name, relative);
        assert!(
            found.len() > 20,
            "walked only {} modules of {name} — the walk is wrong, not the doc",
            found.len()
        );
        modules_seen.extend(found);
        listed.extend(
            named(&doc, &format!("{name}/src/"))
                .into_iter()
                .filter(|m| !m.ends_with(".tests.rs")),
        );
    }
    assert!(
        modules_seen.len() > 200,
        "walked only {} modules across both crates",
        modules_seen.len()
    );

    let missing: Vec<&String> = modules_seen.difference(&listed).collect();
    assert!(
        missing.is_empty(),
        "modules with no row in docs/reference/cockpit-modules.md:\n{}",
        missing
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let stale: Vec<&String> = listed.difference(&modules_seen).collect();
    assert!(
        stale.is_empty(),
        "rows in docs/reference/cockpit-modules.md naming files that are gone:\n{}",
        stale
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The guard column names companion tests by path, and a companion test
/// is deleted far more often than a module is — a row pointing at one
/// that is gone claims a guarantee nothing holds up.
#[test]
fn every_companion_test_the_guard_column_names_exists() {
    let doc = doc();
    let crates_dir = crate_dir("..");
    let mut companions = Vec::new();
    for (name, _) in CRATES {
        companions.extend(
            named(&doc, &format!("{name}/src/"))
                .into_iter()
                .filter(|m| m.ends_with(".tests.rs")),
        );
    }
    assert!(
        companions.len() > 100,
        "the doc names only {} companion tests",
        companions.len()
    );
    let gone: Vec<&String> = companions
        .iter()
        .filter(|m| !crates_dir.join(m).is_file())
        .collect();
    assert!(
        gone.is_empty(),
        "the guard column names companion tests that are gone:\n{}",
        gone.iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_integration_test_of_both_crates_is_named_as_a_guard() {
    let doc = doc();
    let crates_dir = crate_dir("..");
    let mut missing = Vec::new();
    let mut stale = Vec::new();
    for (name, relative) in CRATES {
        let dir = crate_dir(relative).join("tests");
        let mut harnesses = BTreeSet::new();
        for entry in std::fs::read_dir(&dir).expect("read tests/").flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
                harnesses.insert(format!(
                    "{name}/tests/{}",
                    path.file_name().expect("a file name").to_string_lossy()
                ));
            }
        }
        assert!(
            harnesses.len() > 1,
            "walked only {} integration tests of {name}",
            harnesses.len()
        );
        let listed = named(&doc, &format!("{name}/tests/"));
        missing.extend(harnesses.difference(&listed).cloned());
        stale.extend(
            listed
                .difference(&harnesses)
                .filter(|m| !crates_dir.join(m).is_file())
                .cloned(),
        );
    }
    // Live session renderer tests belong to the CLI host, which can construct
    // its supervisor without a TUI -> CLI dependency cycle. Preserve existence
    // checks for those named guards without indexing every unrelated CLI test.
    stale.extend(
        named(&doc, "magistr-cli/tests/")
            .into_iter()
            .filter(|path| !crates_dir.join(path).is_file()),
    );
    assert!(
        missing.is_empty(),
        "integration tests the doc never names:\n{}",
        missing
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        stale.is_empty(),
        "the doc names integration tests that are gone:\n{}",
        stale
            .iter()
            .map(|m| format!("  {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
