use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use regex::Regex;

use super::{
    module_location::ModuleLocation,
    paths::{base_of, dir_of, stem_of},
};

/// The crate's `.rs` files plus the root files grouped by directory.
pub(super) struct ModuleTree {
    pub files: BTreeSet<String>,
    pub manifest_targets: BTreeSet<String>,
    pub roots_by_dir: BTreeMap<String, Vec<String>>,
}

fn auto_root() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^(?:src/(?:lib|main)\.rs|build\.rs|src/bin/[^/]+\.rs|src/bin/[^/]+/main\.rs|(?:tests|examples|benches)/[^/]+\.rs|(?:tests|examples|benches)/[^/]+/main\.rs)$").expect("static regex")
    })
}

/// Is `rel` (crate-relative) a crate-root file — auto-discovered or a manifest target?
pub(super) fn is_root_file(rel: &str, manifest_targets: &BTreeSet<String>) -> bool {
    auto_root().is_match(rel) || manifest_targets.contains(rel)
}

impl ModuleTree {
    pub fn build(files: BTreeSet<String>, manifest_targets: BTreeSet<String>) -> Self {
        let mut roots_by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in &files {
            if is_root_file(file, &manifest_targets) {
                roots_by_dir
                    .entry(dir_of(file).to_owned())
                    .or_default()
                    .push(file.clone());
            }
        }
        Self {
            files,
            manifest_targets,
            roots_by_dir,
        }
    }

    /// Locate a crate-relative `.rs` file by convention: `<dir>/<name>.rs` and
    /// `<dir>/<name>/mod.rs` are module `name` declared by `<dir>.rs` /
    /// `<dir>/mod.rs`, or by the root file(s) when `<dir>` is the root
    /// directory. None when no chain to a root exists (orphan).
    pub fn locate(&self, rel: &str) -> Option<ModuleLocation> {
        let stem = stem_of(rel)?;
        if is_root_file(rel, &self.manifest_targets) {
            return Some(ModuleLocation {
                roots: vec![rel.to_owned()],
                root_dir: dir_of(rel).to_owned(),
                segments: Vec::new(),
                parents: Vec::new(),
            });
        }
        let mut segments: Vec<String> = if stem == "mod" {
            Vec::new()
        } else {
            vec![stem.to_owned()]
        };
        let mut dir = dir_of(rel).to_owned();
        let mut parents: Option<Vec<String>> = None;
        for _ in 0..64 {
            let roots_here: Vec<String> = self
                .roots_by_dir
                .get(&dir)
                .into_iter()
                .flatten()
                .filter(|root| *root != rel)
                .cloned()
                .collect();
            if !roots_here.is_empty() {
                return Some(ModuleLocation {
                    parents: parents.unwrap_or_else(|| roots_here.clone()),
                    roots: roots_here,
                    root_dir: dir,
                    segments,
                });
            }
            if dir.is_empty() {
                return None;
            }
            let name = base_of(&dir).to_owned();
            let candidates: Vec<String> = [format!("{dir}.rs"), format!("{dir}/mod.rs")]
                .into_iter()
                .filter(|candidate| candidate != rel && self.files.contains(candidate))
                .collect();
            if candidates.is_empty() {
                // `<dir>/mod.rs` IS this directory's module: ascend without a parent yet.
                if format!("{dir}/mod.rs") == rel {
                    segments.insert(0, name);
                    dir = dir_of(&dir).to_owned();
                    continue;
                }
                return None;
            }
            // `tests/foo.rs` beside `tests/foo/helper.rs`: the sibling file IS a root.
            let root_candidates: Vec<String> = candidates
                .iter()
                .filter(|candidate| is_root_file(candidate, &self.manifest_targets))
                .cloned()
                .collect();
            if !root_candidates.is_empty() {
                return Some(ModuleLocation {
                    parents: parents.unwrap_or_else(|| root_candidates.clone()),
                    roots: root_candidates,
                    root_dir: dir_of(&dir).to_owned(),
                    segments,
                });
            }
            parents.get_or_insert(candidates);
            segments.insert(0, name);
            dir = dir_of(&dir).to_owned();
        }
        None
    }
}

#[cfg(test)]
#[path = "module_tree.tests.rs"]
mod tests;
