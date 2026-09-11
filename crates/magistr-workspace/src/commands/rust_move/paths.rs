use std::collections::BTreeSet;

pub(super) fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

pub(super) fn base_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, base)| base)
}

pub(super) fn join_path(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

pub(super) fn stem_of(path: &str) -> Option<&str> {
    base_of(path).strip_suffix(".rs")
}

pub(super) fn is_under(path: &str, dir: &str) -> bool {
    dir.is_empty() || path == dir || path.starts_with(&format!("{dir}/"))
}

pub(super) fn relative_to<'p>(path: &'p str, dir: &str) -> &'p str {
    if dir.is_empty() {
        path
    } else {
        path.get(dir.len() + 1..).unwrap_or("")
    }
}

/// The directory whose `<dir>.rs` / `<dir>/mod.rs` (or root file) declares `rel`.
pub(super) fn parent_dir_of(rel: &str) -> &str {
    if stem_of(rel) == Some("mod") {
        dir_of(dir_of(rel))
    } else {
        dir_of(rel)
    }
}

/// The nearest ancestor directory holding a Cargo.toml, else "" (workspace root).
pub(super) fn crate_dir_of(path: &str, files: &BTreeSet<String>) -> String {
    let mut dir = dir_of(path);
    loop {
        if files.contains(&join_path(dir, "Cargo.toml")) {
            return dir.to_owned();
        }
        if dir.is_empty() {
            return String::new();
        }
        dir = dir_of(dir);
    }
}
