use std::collections::BTreeSet;

use super::source_reads::SourceReads;
use crate::{file_content::FileContent, query::SourceEntry};

/// One importer rewritten for a move.
pub(super) struct ImporterWrite {
    pub path: String,
    pub before: String,
    pub content: String,
}

/// Every specifier rewrite a non-Rust move needs. Specifier-based and
/// name-resolution-free: a specifier is rewritten only when it resolves, by
/// the family's path convention, to exactly the moved file.
pub(super) struct MovePlan {
    pub moved_content: String,
    pub moved_count: usize,
    pub importers: Vec<ImporterWrite>,
    pub importers_rewritten: usize,
    pub specifiers_rewritten: usize,
}

pub(super) const JS_EXTS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
pub(super) const C_EXTS: [&str; 7] = ["c", "h", "cpp", "cc", "cxx", "hpp", "hh"];

pub(super) fn ext_of(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(path, |(_, base)| base)
        .rsplit_once('.')
        .map_or(String::new(), |(_, ext)| ext.to_ascii_lowercase())
}

pub(super) fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Posix-relative path from a workspace-relative dir to a workspace-relative file.
pub(super) fn relative_posix(from_dir: &str, to_path: &str) -> String {
    let a: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').collect()
    };
    let b: Vec<&str> = to_path.split('/').collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; a.len() - common];
    parts.extend_from_slice(&b[common..]);
    parts.join("/")
}

/// Join a workspace-relative dir with a ./ or ../ specifier, resolving dots.
pub(super) fn join_rel(from_dir: &str, specifier: &str) -> String {
    let mut parts: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').collect()
    };
    for part in specifier.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// Which importer files to scan for specifiers, per moved-file extension.
fn importer_extensions(moved_ext: &str) -> &'static [&'static str] {
    if JS_EXTS.contains(&moved_ext) || moved_ext == "json" {
        &JS_EXTS
    } else if moved_ext == "py" {
        &["py"]
    } else if moved_ext == "java" {
        &["java"]
    } else if C_EXTS.contains(&moved_ext) {
        &C_EXTS
    } else {
        &[]
    }
}

fn rewrite_importer(
    importer: &str,
    content: &str,
    from: &str,
    to: &str,
    files: &BTreeSet<String>,
) -> (String, usize) {
    let moved_ext = ext_of(from);
    if JS_EXTS.contains(&moved_ext.as_str()) || moved_ext == "json" {
        super::move_imports_js::rewrite_importer(importer, content, from, to, files)
    } else if moved_ext == "py" {
        super::move_imports_python::rewrite_importer(content, from, to)
    } else if moved_ext == "java" {
        super::move_imports_java::rewrite_importer(content, from, to)
    } else if C_EXTS.contains(&moved_ext.as_str()) {
        super::move_imports_c::rewrite_importer(importer, content, from, to)
    } else {
        (content.to_owned(), 0)
    }
}

fn rewrite_moved(from: &str, to: &str, content: &str, files: &BTreeSet<String>) -> (String, usize) {
    let moved_ext = ext_of(from);
    if JS_EXTS.contains(&moved_ext.as_str()) {
        super::move_imports_js::rewrite_moved(from, to, content, files)
    } else if C_EXTS.contains(&moved_ext.as_str()) {
        super::move_imports_c::rewrite_moved(from, to, content, files)
    } else {
        // Python absolute modules and java imports do not change with the
        // mover's own location; their importer-side rewrites carry the move.
        (content.to_owned(), 0)
    }
}

pub(super) fn plan(
    reads: &SourceReads<'_>,
    listing: &[SourceEntry],
    from: &str,
    to: &str,
    source: &FileContent,
) -> MovePlan {
    let files: BTreeSet<String> = listing
        .iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.path.clone())
        .collect();
    let (moved_content, moved_count) = match source {
        FileContent::Text(text) => rewrite_moved(from, to, text, &files),
        FileContent::Binary(_) => (String::new(), 0),
    };
    let scan = importer_extensions(&ext_of(from));
    let mut plan = MovePlan {
        moved_content,
        moved_count,
        importers: Vec::new(),
        importers_rewritten: 0,
        specifiers_rewritten: moved_count,
    };
    for entry in listing {
        if entry.is_dir
            || entry.junk
            || entry.path == from
            || entry.path == to
            || !scan.contains(&ext_of(&entry.path).as_str())
        {
            continue;
        }
        let Ok(Some(FileContent::Text(before))) = reads.read(&entry.path) else {
            continue;
        };
        let (content, count) = rewrite_importer(&entry.path, &before, from, to, &files);
        if count == 0 {
            continue;
        }
        plan.importers_rewritten += 1;
        plan.specifiers_rewritten += count;
        plan.importers.push(ImporterWrite {
            path: entry.path.clone(),
            before,
            content,
        });
    }
    plan
}

#[cfg(test)]
#[path = "move_imports.tests.rs"]
mod tests;
