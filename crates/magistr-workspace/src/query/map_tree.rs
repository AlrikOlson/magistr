use super::{MapRow, source_entry::SourceEntry};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct MapTree {
    pub rows: Vec<MapRow>,
    pub files: usize,
    pub dirs: usize,
    pub languages: BTreeMap<String, usize>,
}

impl MapTree {
    pub fn build(entries: &[SourceEntry], prefix: &str, depth: usize) -> Self {
        let scoped = entries
            .iter()
            .filter(|entry| !entry.is_dir)
            .filter_map(|entry| {
                let rel = if prefix.is_empty() {
                    entry.path.as_str()
                } else {
                    entry.path.strip_prefix(prefix)?.strip_prefix('/')?
                };
                Some((entry, rel))
            })
            .collect::<Vec<_>>();
        let mut dir_files = BTreeMap::<String, usize>::new();
        let mut dir_children = BTreeMap::<String, BTreeSet<String>>::new();
        let mut languages = BTreeMap::<String, usize>::new();
        for (_, rel) in &scoped {
            let parts = rel.split('/').collect::<Vec<_>>();
            for i in 1..parts.len() {
                let dir = parts[..i].join("/");
                *dir_files.entry(dir.clone()).or_default() += 1;
                if i >= 2 {
                    dir_children
                        .entry(parts[..i - 1].join("/"))
                        .or_default()
                        .insert(dir);
                }
            }
            if let Some(language) = language_of(rel) {
                *languages.entry(language).or_default() += 1;
            }
        }
        let mut rows = dir_files
            .iter()
            .filter(|(dir, _)| dir.split('/').count() <= depth)
            .map(|(dir, files)| MapRow::Dir {
                path: if prefix.is_empty() {
                    dir.clone()
                } else {
                    format!("{prefix}/{dir}")
                },
                files: *files,
                dirs: dir_children.get(dir).map_or(0, BTreeSet::len),
            })
            .collect::<Vec<_>>();
        rows.extend(
            scoped
                .iter()
                .filter(|(_, rel)| rel.split('/').count() <= depth)
                .map(|(entry, rel)| MapRow::File {
                    path: entry.path.clone(),
                    junk: entry.junk,
                    language: language_of(rel),
                    symbols: Vec::new(),
                    dropped_symbols: None,
                }),
        );
        rows.sort_by(|left, right| {
            left.path()
                .split('/')
                .count()
                .cmp(&right.path().split('/').count())
                .then_with(|| {
                    matches!(left, MapRow::File { .. }).cmp(&matches!(right, MapRow::File { .. }))
                })
                .then_with(|| left.path().cmp(right.path()))
        });
        Self {
            rows,
            files: scoped.len(),
            dirs: dir_files.len(),
            languages,
        }
    }
}

fn language_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    let extension = extension.to_lowercase();
    Some(
        match extension.as_str() {
            "sh" | "bash" => "bash",
            "c" | "h" => "c",
            "cc" | "cpp" | "cxx" | "hh" | "hpp" => "cpp",
            "cs" => "csharp",
            "go" => "go",
            "java" => "java",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "json" => "json",
            "py" => "python",
            "rb" => "ruby",
            "rs" => "rust",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "tsx",
            "md" => "markdown",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            other => other,
        }
        .to_owned(),
    )
}

#[cfg(test)]
#[path = "map_tree.tests.rs"]
mod tests;
