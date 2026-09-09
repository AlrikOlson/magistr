use super::*;

use std::path::PathBuf;

use magistr_core::types::{ProjectRole, TestingConfig};
use magistr_core::{FormatConfig, QualityGate};

const WORKSPACE_FMT: &str = "cargo fmt --manifest-path rust/Cargo.toml";

fn arch(id: &str, root: &str, command: &str) -> Archetype {
    Archetype {
        id: id.to_owned(),
        name: id.to_owned(),
        language: "rust".to_owned(),
        framework: None,
        root: PathBuf::from(root),
        role: ProjectRole::Unknown,
        quality_gate: QualityGate {
            check: String::new(),
            test: String::new(),
            lint: String::new(),
        },
        format: FormatConfig {
            command: command.to_owned(),
            extensions: vec!["rs".to_owned()],
            exclusions: vec!["target".to_owned()],
        },
        testing: testing(),
        source_patterns: vec![],
        test_patterns: vec![],
        config_patterns: vec![],
        naming_convention: None,
        module_structure: None,
        model_preference: None,
    }
}

fn testing() -> TestingConfig {
    TestingConfig {
        companion_pattern: "{name}.tests.{ext}".to_owned(),
        test_dir: "alongside".to_owned(),
    }
}

fn root() -> PathBuf {
    PathBuf::from("/proj")
}

/// `shar-remaster`, exactly: eight archetypes that all carry the project's own
/// workspace-wide `cargo fmt`. That is one formatter, at the project root —
/// not eight formatters in eight sub-directories, where the command's
/// root-relative `--manifest-path` cannot resolve and every call fails.
#[test]
fn every_archetype_carrying_the_projects_own_command_is_one_run_at_the_root() {
    let archetypes = [
        arch("rust", "rust", WORKSPACE_FMT),
        arch("shar", "rust/apps/shar", WORKSPACE_FMT),
        arch("sim", "rust/crates/sim", WORKSPACE_FMT),
    ];
    let plan = FormatRuns::plan(&root(), WORKSPACE_FMT, &archetypes, None);
    assert_eq!(plan.runs().len(), 1, "one command is one run");
    let run = &plan.runs()[0];
    assert_eq!(run.dir, root(), "the project's command runs where it works");
    assert_eq!(run.archetypes, ["rust", "shar", "sim"], "all of them");
}

/// A real per-archetype formatter still runs in its own root — that is what an
/// archetype is, and a `web/` formatter must not be invoked from the repo root.
#[test]
fn an_archetypes_own_command_runs_in_its_own_root() {
    let archetypes = [
        arch("rust", "rust", WORKSPACE_FMT),
        arch("web", "web", "npm run format"),
    ];
    let plan = FormatRuns::plan(&root(), WORKSPACE_FMT, &archetypes, None);
    assert_eq!(plan.runs().len(), 2);
    assert_eq!(plan.runs()[0].dir, root());
    assert_eq!(plan.runs()[1].dir, root().join("web"));
}

/// Two archetypes with the same command in the same directory are one run.
/// They were spawned in parallel, so this is not thrift — it is two processes
/// writing the same files at the same time.
#[test]
fn the_same_command_in_the_same_directory_is_never_run_twice() {
    let archetypes = [
        arch("a", "svc", "prettier --write ."),
        arch("b", "svc", "prettier --write ."),
    ];
    let plan = FormatRuns::plan(&root(), WORKSPACE_FMT, &archetypes, None);
    assert_eq!(plan.runs().len(), 1);
    assert_eq!(plan.runs()[0].archetypes, ["a", "b"]);
}

/// A named file list reaches the run rather than being dropped — the
/// all-archetypes path used to pass `None` and format the whole project
/// however few files the caller asked for.
#[test]
fn a_named_file_list_reaches_the_run() {
    let archetypes = [arch("rust", "rust", WORKSPACE_FMT)];
    let plan = FormatRuns::plan(
        &root(),
        WORKSPACE_FMT,
        &archetypes,
        Some(&["rust/src/a.rs"]),
    );
    assert_eq!(
        plan.runs()[0].files.as_deref(),
        Some(&["rust/src/a.rs".to_owned()][..])
    );
    assert!(!plan.runs()[0].is_empty());
}

/// And it is split by root: the Rust formatter is never handed a TypeScript
/// file, and an archetype with none of the named files is not run at all.
#[test]
fn a_file_list_is_split_by_archetype_and_an_untouched_archetype_does_not_run() {
    let archetypes = [
        arch("web", "web", "npm run format"),
        arch("api", "api", "black ."),
    ];
    let plan = FormatRuns::plan(&root(), WORKSPACE_FMT, &archetypes, Some(&["web/app.ts"]));
    assert_eq!(
        plan.runs()[0].files.as_deref(),
        Some(&["web/app.ts".to_owned()][..])
    );
    assert!(!plan.runs()[0].is_empty());
    assert!(
        plan.runs()[1].is_empty(),
        "the python formatter has nothing to do"
    );
}

/// A run at the project root takes every named file, whichever archetype the
/// file belongs to: the project's own command covers the whole tree.
#[test]
fn a_run_at_the_project_root_takes_every_named_file() {
    let archetypes = [arch("rust", "rust/crates/sim", WORKSPACE_FMT)];
    let plan = FormatRuns::plan(
        &root(),
        WORKSPACE_FMT,
        &archetypes,
        Some(&["rust/crates/sim/a.rs", "rust/apps/shar/b.rs"]),
    );
    assert_eq!(
        plan.runs()[0].files.as_deref(),
        Some(
            &[
                "rust/crates/sim/a.rs".to_owned(),
                "rust/apps/shar/b.rs".to_owned()
            ][..]
        )
    );
}

/// An archetype rooted at the repository itself contains everything.
#[test]
fn an_archetype_rooted_at_the_project_contains_every_file() {
    assert!(under(Path::new("src/a.rs"), Path::new(".")));
    assert!(under(Path::new("src/a.rs"), Path::new("")));
    assert!(under(Path::new("rust/src/a.rs"), Path::new("rust")));
    assert!(!under(Path::new("web/a.ts"), Path::new("rust")));
}
