use std::{collections::BTreeSet, sync::OnceLock};

use regex::Regex;

use super::declaration_item::DeclarationItem;

pub(super) const VIS: &str = r"(?:pub(?:\([^)]*\))?[ \t]+)?";
pub(super) const IDENT: &str = r"[A-Za-z_](?-u:\w)*";

/// Module names declared in this file — `mod x;` and inline `mod x {`.
pub(super) fn declared_modules(content: &str) -> BTreeSet<String> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(&format!(r"(?m)^[ \t]*{VIS}mod[ \t]+({IDENT})[ \t]*[;{{]"))
            .expect("static regex")
    });
    pattern
        .captures_iter(content)
        .map(|captures| captures[1].to_owned())
        .collect()
}

fn declaration_line(name: &str) -> Regex {
    Regex::new(&format!(
        r"^[ \t]*({VIS})mod[ \t]+{}[ \t]*;",
        regex::escape(name)
    ))
    .expect("escaped name regex")
}

/// Find the `mod <name>;` item (file module, not inline) with its leading attributes/docs.
pub(super) fn find_declaration(content: &str, name: &str) -> Option<DeclarationItem> {
    static PATH_ATTR: OnceLock<Regex> = OnceLock::new();
    let path_attr = PATH_ATTR.get_or_init(|| Regex::new(r"^#\[\s*path\s*=").expect("static regex"));
    let pattern = declaration_line(name);
    let lines: Vec<&str> = content.split('\n').collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(captures) = pattern.captures(line) else {
            continue;
        };
        let mut start = index;
        let mut has_path_attr = false;
        while start > 0 {
            let previous = lines[start - 1].trim();
            if !(previous.starts_with("#[") || previous.starts_with("///")) {
                break;
            }
            if path_attr.is_match(previous) {
                has_path_attr = true;
            }
            start -= 1;
        }
        return Some(DeclarationItem {
            start_line: start,
            decl_line: index,
            has_path_attr,
            is_pub: !captures[1].trim().is_empty(),
        });
    }
    None
}

/// Rename the `mod old;` item in place, keeping everything else on its lines.
pub(super) fn rename_declaration(content: &str, old_name: &str, new_name: &str) -> String {
    let pattern = Regex::new(&format!(
        r"(?m)^([ \t]*{VIS}mod[ \t]+){}([ \t]*;)",
        regex::escape(old_name)
    ))
    .expect("escaped name regex");
    pattern
        .replacen(content, 1, |captures: &regex::Captures<'_>| {
            format!("{}{new_name}{}", &captures[1], &captures[2])
        })
        .into_owned()
}

/// Remove the item's lines (attributes/docs included).
pub(super) fn remove_declaration(content: &str, item: &DeclarationItem) -> String {
    let mut lines: Vec<&str> = content.split('\n').collect();
    lines.drain(item.start_line..=item.decl_line);
    lines.join("\n")
}

/// The item's own lines, flush-left, reusable in another parent.
pub(super) fn declaration_item_text(content: &str, item: &DeclarationItem) -> String {
    content
        .split('\n')
        .skip(item.start_line)
        .take(item.decl_line + 1 - item.start_line)
        .map(|line| line.trim_start_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert a declaration item: after the last file-module `mod x;` line, else
/// after the leading inner-attribute / inner-doc block (which must stay first).
pub(super) fn insert_declaration(content: &str, item: &str) -> String {
    static MOD_LINE: OnceLock<Regex> = OnceLock::new();
    let mod_line = MOD_LINE.get_or_init(|| {
        Regex::new(&format!(r"^[ \t]*{VIS}mod[ \t]+{IDENT}[ \t]*;")).expect("static regex")
    });
    let mut lines: Vec<String> = content.split('\n').map(str::to_owned).collect();
    let mut item_lines: Vec<String> = item.split('\n').map(str::to_owned).collect();
    if let Some(last) = lines.iter().rposition(|line| mod_line.is_match(line)) {
        lines.splice(last + 1..last + 1, item_lines);
        return lines.join("\n");
    }
    let mut at = 0;
    while at < lines.len() {
        let trimmed = lines[at].trim();
        if trimmed.starts_with("//!")
            || trimmed.starts_with("#![")
            || (trimmed.is_empty() && at > 0)
        {
            at += 1;
        } else {
            break;
        }
    }
    if at > 0 && !lines[at - 1].trim().is_empty() {
        item_lines.insert(0, String::new());
    }
    item_lines.push(String::new());
    lines.splice(at..at, item_lines);
    lines.join("\n")
}

#[cfg(test)]
#[path = "declarations.tests.rs"]
mod tests;
