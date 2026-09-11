use std::{collections::BTreeSet, sync::OnceLock};

use regex::Regex;

use super::move_imports::{JS_EXTS, dir_of, ext_of, join_rel, relative_posix};
use super::source_reads::SourceReads;

/// import/export-from, bare import, dynamic import() and require() specifiers.
fn patterns() -> &'static [Regex; 4] {
    static PATTERNS: OnceLock<[Regex; 4]> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            Regex::new(
                r#"((?:^|\n)[^\n"'`]*?(?:import|export)(?-u:\b)[^\n"'`]*?(?-u:\b)from\s*["'])([^"'\n]+)(["'])"#,
            )
            .expect("static regex"),
            Regex::new(r#"((?:^|\n)\s*import\s*["'])([^"'\n]+)(["'])"#).expect("static regex"),
            Regex::new(r#"((?-u:\b)import\s*\(\s*["'])([^"'\n]+)(["']\s*\))"#)
                .expect("static regex"),
            Regex::new(r#"((?-u:\b)require\s*\(\s*["'])([^"'\n]+)(["']\s*\))"#)
                .expect("static regex"),
        ]
    })
}

/// Replace capture group 2 of every `pattern` match via `next`; count changes.
pub(super) fn rewrite_specifiers(
    content: &str,
    pattern: &Regex,
    mut next: impl FnMut(&str) -> Option<String>,
) -> (String, usize) {
    let mut count = 0;
    let rewritten = pattern.replace_all(content, |captures: &regex::Captures<'_>| {
        let spec = &captures[2];
        match next(spec) {
            Some(replacement) if replacement != spec => {
                count += 1;
                format!("{}{replacement}{}", &captures[1], &captures[3])
            }
            _ => captures[0].to_owned(),
        }
    });
    (rewritten.into_owned(), count)
}

/// Resolve a ts/js relative specifier from `from_dir` to a workspace path.
fn resolve_specifier(from_dir: &str, specifier: &str, files: &BTreeSet<String>) -> Option<String> {
    if !specifier.starts_with('.') {
        return None;
    }
    let base = SourceReads::normalize(&join_rel(from_dir, specifier));
    if files.contains(&base) {
        return Some(base);
    }
    for ext in JS_EXTS {
        let with_ext = format!("{base}.{ext}");
        if files.contains(&with_ext) {
            return Some(with_ext);
        }
        let index = format!("{base}/index.{ext}");
        if files.contains(&index) {
            return Some(index);
        }
    }
    None
}

/// The ts/js specifier for `target` seen from `from_dir`, in `like`'s style.
fn specifier_for(from_dir: &str, target: &str, like: &str) -> String {
    let explicit_ext = like
        .rsplit_once('.')
        .is_some_and(|(_, ext)| !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()));
    let mut path = relative_posix(from_dir, target);
    if !explicit_ext {
        let ext = ext_of(target);
        if JS_EXTS.contains(&ext.as_str()) {
            path.truncate(path.len() - ext.len() - 1);
        }
    }
    if path.starts_with('.') {
        path
    } else {
        format!("./{path}")
    }
}

/// Rewrite ts/js specifiers in one importer that resolve to `from` → `to`.
pub(super) fn rewrite_importer(
    importer: &str,
    content: &str,
    from: &str,
    to: &str,
    files: &BTreeSet<String>,
) -> (String, usize) {
    let from_dir = dir_of(importer);
    let mut total = 0;
    let mut current = content.to_owned();
    for pattern in patterns() {
        let (next, count) = rewrite_specifiers(&current, pattern, |spec| {
            (resolve_specifier(from_dir, spec, files).as_deref() == Some(from))
                .then(|| specifier_for(from_dir, to, spec))
        });
        current = next;
        total += count;
    }
    (current, total)
}

/// Recompute the moved ts/js file's own relative specifiers from its new dir.
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
    let mut total = 0;
    let mut current = content.to_owned();
    for pattern in patterns() {
        let (next, count) = rewrite_specifiers(&current, pattern, |spec| {
            let target = resolve_specifier(old_dir, spec, files)?;
            // A self-import cannot survive the move; importer rewriting owns `from`.
            (target != from).then(|| specifier_for(new_dir, &target, spec))
        });
        current = next;
        total += count;
    }
    (current, total)
}
