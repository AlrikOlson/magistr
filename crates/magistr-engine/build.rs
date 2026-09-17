//! The owner contract identity, fixed at compile time.
//!
//! Every type that crosses the owner socket — `Hello`, `OwnerCommand`,
//! `OwnerReply` and the workflow domain values inside them — is defined by
//! this crate and its path dependencies. Their source, together with the
//! workspace lock file that pins every other dependency, is digested here
//! into `MAGISTR_OWNER_CONTRACT_SOURCE`, beside the build profile and the
//! target triple. Two binaries that agree on all of them speak the same
//! contract; two that differ are told apart even when their package
//! versions are equal. The digest is over source, so it is the same in every
//! checkout of the same tree and independent of where that tree lives.
//!
//! The digested set is each crate's manifest and its `src/` tree less the
//! unit-test files, which never ship. Each file is its own rerun trigger:
//! a change to any of them already recompiles this crate, while a test,
//! bench, document or stray file under those crates reruns nothing — a
//! directory-level trigger would re-check this crate and every dependent
//! on any mtime change under it.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Directories under `src/` that hold generated or run-time state rather
/// than source: never part of the contract. Hidden entries (editor state,
/// `.DS_Store`) are skipped for the same reason, so two checkouts of one
/// tree agree whatever their tooling left behind.
const SKIPPED_DIRECTORIES: &[&str] = &["target", "proptest-regressions"];

/// Unit tests live beside their module as `<name>.tests.rs` and are compiled
/// only into test binaries: not part of any shipped contract.
const UNIT_TEST_SUFFIX: &str = ".tests.rs";

fn main() {
    let manifest_dir = PathBuf::from(required("CARGO_MANIFEST_DIR"));
    let manifest_dir = manifest_dir
        .canonicalize()
        .unwrap_or_else(|error| panic!("cannot resolve {}: {error}", manifest_dir.display()));
    let mut crates = BTreeSet::new();
    collect_path_dependencies(&manifest_dir, &mut crates);

    // The digest names files relative to the workspace root so that two
    // checkouts of one tree agree; a crate outside a `crates/` layout is
    // named relative to its own parent instead.
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest_dir.clone());

    let mut files = Vec::new();
    for krate in &crates {
        files.push(krate.join("Cargo.toml"));
        collect_source_files(&krate.join("src"), &mut files);
    }
    let lock = workspace_root.join("Cargo.lock");
    if lock.is_file() {
        files.push(lock);
    }
    files.sort();

    let mut hasher = Sha256::new();
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        digest_file(&workspace_root, file, &mut hasher);
    }
    let digest = hasher.finalize();
    let contract: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();

    println!("cargo:rustc-env=MAGISTR_OWNER_CONTRACT_SOURCE={contract}");
    println!(
        "cargo:rustc-env=MAGISTR_OWNER_BUILD_PROFILE={}",
        required("PROFILE")
    );
    println!(
        "cargo:rustc-env=MAGISTR_OWNER_BUILD_TARGET={}",
        required("TARGET")
    );
}

fn required(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("cargo sets {key} for build scripts"))
}

/// This crate plus, transitively, every `path` dependency named in a
/// `[dependencies]` or `[target.<cfg>.dependencies]` table of its manifest.
/// Development and build dependencies are not part of the binary and are
/// left out.
fn collect_path_dependencies(crate_dir: &Path, seen: &mut BTreeSet<PathBuf>) {
    if !seen.insert(crate_dir.to_path_buf()) {
        return;
    }
    let manifest = crate_dir.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", manifest.display()));
    let mut in_dependency_table = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependency_table = is_dependency_table(line);
            continue;
        }
        if !in_dependency_table {
            continue;
        }
        let Some(path) = path_value(line) else {
            continue;
        };
        let dependency = crate_dir.join(path);
        let dependency = dependency.canonicalize().unwrap_or_else(|error| {
            panic!(
                "{} names path dependency {} which cannot be resolved: {error}",
                manifest.display(),
                dependency.display()
            )
        });
        collect_path_dependencies(&dependency, seen);
    }
}

/// Whether a `[...]` table header opens a table whose entries ship in the
/// binary: `[dependencies]`, `[dependencies.<name>]` and the
/// target-specific forms. `dev-` and `build-` tables do not match.
fn is_dependency_table(header: &str) -> bool {
    let body = header.trim_start_matches('[').trim_end_matches(']');
    body == "dependencies"
        || body.starts_with("dependencies.")
        || (body.starts_with("target.")
            && (body.ends_with(".dependencies") || body.contains(".dependencies.")))
}

/// The value of a `path = "..."` key on a manifest line, whether the line is
/// the key alone or an inline table such as `x = { path = "../x" }`.
fn path_value(line: &str) -> Option<&str> {
    let (_, rest) = line.split_once("path")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let (value, _) = rest.split_once('"')?;
    Some(value)
}

/// Every shipped source file under `dir`: regular files that are not hidden
/// and not unit tests, below directories that are not hidden and not
/// generated state.
fn collect_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", dir.display()))
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", dir.display()));
    for path in entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if !SKIPPED_DIRECTORIES.contains(&name) {
                collect_source_files(&path, out);
            }
        } else if path.is_file() && !name.ends_with(UNIT_TEST_SUFFIX) {
            out.push(path);
        }
    }
}

fn digest_file(root: &Path, path: &Path, hasher: &mut Sha256) {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let bytes =
        fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    hasher.update(relative.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(&bytes);
}
