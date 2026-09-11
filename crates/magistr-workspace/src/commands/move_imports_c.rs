use std::{collections::BTreeSet, sync::OnceLock};

use regex::Regex;

use super::{
    move_imports::{dir_of, join_rel, relative_posix},
    move_imports_js::rewrite_specifiers,
    source_reads::SourceReads,
};

fn include_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?m)(^\s*#\s*include\s*")([^"\n]+)(")"#).expect("static regex")
    })
}

/// Rewrite quoted #include lines in one c/c++ importer: importer-relative
/// first (the compiler's quoted-include order), then the root-relative form.
pub(super) fn rewrite_importer(
    importer: &str,
    content: &str,
    from: &str,
    to: &str,
) -> (String, usize) {
    let from_dir = dir_of(importer);
    rewrite_specifiers(content, include_pattern(), |spec| {
        if SourceReads::normalize(&join_rel(from_dir, spec)) == from {
            Some(relative_posix(from_dir, to))
        } else if SourceReads::normalize(spec) == from {
            Some(to.to_owned())
        } else {
            None
        }
    })
}

/// Recompute the moved c/c++ file's own quoted includes from its new dir.
/// Root-relative includes survive a move unchanged; only importer-relative
/// ones need recomputing, and only when they name a real workspace file.
pub(super) fn rewrite_moved(
    from: &str,
    to: &str,
    content: &str,
    files: &BTreeSet<String>,
) -> (String, usize) {
    let old_dir = dir_of(from);
    let new_dir = dir_of(to);
    if old_dir == new_dir {
        return (content.to_owned(), 0);
    }
    rewrite_specifiers(content, include_pattern(), |spec| {
        let importer_relative = SourceReads::normalize(&join_rel(old_dir, spec));
        if !files.contains(&importer_relative)
            || files.contains(&SourceReads::normalize(spec))
            || importer_relative == from
        {
            return None;
        }
        Some(relative_posix(new_dir, &importer_relative))
    })
}
