//! Structural guard: every `git` spawned for a cockpit surface (the
//! launcher's project/git cards in `home_data`) must pipe stdout and null
//! stderr, and never use `status()`/`spawn()` with inherited stdio.
//!
//! With inherited stdio, git's stderr ("fatal: not a git repository", …)
//! is written straight onto the alternate screen and corrupts the frame.
//! `Command::output()` pipes both streams by default, but an explicit
//! `.stderr(Stdio::null())` keeps the intent visible and survives a later
//! switch to `status()`/`spawn()`; hence the test checks the source text.

use std::fs;
use std::path::{Path, PathBuf};

const SPAWN_MARKER: &str = "Command::new(\"git\")";
const TERMINALS: [&str; 3] = [".output()", ".status()", ".spawn()"];

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every `Command::new("git")` builder chain in `src`, as
/// `(file, line, chain text up to and including its terminal call)`.
fn git_chains(src: &str, file: &Path) -> Vec<(String, usize, String)> {
    let mut chains = Vec::new();
    let mut from = 0;
    while let Some(rel) = src[from..].find(SPAWN_MARKER) {
        let start = from + rel;
        let rest = &src[start..];
        let end = TERMINALS
            .iter()
            .filter_map(|t| rest.find(t).map(|i| i + t.len()))
            .min()
            .unwrap_or(rest.len());
        let line = src[..start].matches('\n').count() + 1;
        chains.push((file.display().to_string(), line, rest[..end].to_string()));
        from = start + end;
    }
    chains
}

#[test]
fn every_cockpit_git_spawn_pipes_stdout_and_nulls_stderr() {
    let feeder =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/orchestration/commands/home_data.rs");
    let mut files = vec![feeder];
    let tui_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../magistr-tui/src");
    rust_sources(&tui_crate, &mut files);

    let mut chains = Vec::new();
    for file in &files {
        let src = fs::read_to_string(file).expect("read source");
        chains.extend(git_chains(&src, file));
    }
    assert!(
        !chains.is_empty(),
        "expected at least one git spawn in the cockpit feeders — did the marker change?"
    );

    let mut violations = Vec::new();
    for (file, line, chain) in &chains {
        let mut problems = Vec::new();
        if !chain.contains(".stderr(std::process::Stdio::null())")
            && !chain.contains(".stderr(Stdio::null())")
        {
            problems.push("stderr not nulled");
        }
        if !chain.contains(".stdout(std::process::Stdio::piped())")
            && !chain.contains(".stdout(Stdio::piped())")
        {
            problems.push("stdout not piped");
        }
        if chain.ends_with(".status()") || chain.ends_with(".spawn()") {
            problems.push("uses status()/spawn() (inherited stdio)");
        }
        if !problems.is_empty() {
            violations.push(format!("{file}:{line}: {}", problems.join(", ")));
        }
    }

    assert!(
        violations.is_empty(),
        "cockpit git spawns with leaky stdio:\n{}",
        violations.join("\n")
    );
}

#[test]
fn chain_extraction_finds_each_spawn_and_stops_at_its_terminal() {
    let src = r#"
        let a = std::process::Command::new("git").args(["x"]).stdout(Stdio::piped()).stderr(Stdio::null()).output();
        let b = std::process::Command::new("git").args(["y"]).status();
    "#;
    let chains = git_chains(src, Path::new("fake.rs"));
    assert_eq!(chains.len(), 2);
    assert!(chains[0].2.ends_with(".output()"));
    assert!(!chains[0].2.contains("args([\"y\"]"));
    assert!(chains[1].2.ends_with(".status()"));
    assert_eq!(chains[1].1, 3);
}
