use std::sync::OnceLock;

use regex::Regex;

use super::move_imports::ext_of;

/// The absolute dotted module a python file answers to (a/b.py → a.b).
fn module_of(path: &str) -> Option<String> {
    if ext_of(path) != "py" {
        return None;
    }
    let stem = &path[..path.len() - 3];
    let base = stem.strip_suffix("/__init__").unwrap_or(stem);
    Some(base.replace('/', "."))
}

fn is_module_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// Rewrite absolute `import a.b` / `from a.b import x` lines in one importer.
/// The retained rule rewrites the first boundary-delimited occurrence on each
/// import line, before any comment.
pub(super) fn rewrite_importer(content: &str, from: &str, to: &str) -> (String, usize) {
    static HEAD: OnceLock<Regex> = OnceLock::new();
    let head = HEAD.get_or_init(|| Regex::new(r"^\s*(?:from|import)\s").expect("static regex"));
    let (Some(old_module), Some(new_module)) = (module_of(from), module_of(to)) else {
        return (content.to_owned(), 0);
    };
    let mut count = 0;
    let mut out = String::with_capacity(content.len());
    for (index, line) in content.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        if head.is_match(line) {
            let scan_end = line.find('#').unwrap_or(line.len());
            let mut search = 0;
            let mut replaced = false;
            while let Some(offset) = line[search..scan_end].find(&old_module) {
                let at = search + offset;
                let end = at + old_module.len();
                let before_ok = !line[..at].chars().next_back().is_some_and(is_module_char);
                let after_ok = !line[end..].chars().next().is_some_and(is_module_char);
                if before_ok && after_ok {
                    out.push_str(&line[..at]);
                    out.push_str(&new_module);
                    out.push_str(&line[end..]);
                    count += 1;
                    replaced = true;
                    break;
                }
                search = at + 1;
            }
            if replaced {
                continue;
            }
        }
        out.push_str(line);
    }
    (out, count)
}
