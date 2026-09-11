use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use regex::Regex;

use super::{declarations::IDENT, rewrite_context::RewriteContext};

/// A `use` binding in one file: local name → absolute module path (same crate).
#[derive(Clone, Debug)]
struct Binding {
    abs: Vec<String>,
    aliased: bool,
    via_group: bool,
}

fn use_single() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(&format!(
            r"(?-u:\b)use[ \t]+((?:crate|super|self|{IDENT})(?:::(?:super|self|{IDENT}))*)(?:[ \t]+as[ \t]+({IDENT}))?[ \t]*;"
        ))
        .expect("static regex")
    })
}

fn use_group() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(&format!(
            r"(?-u:\b)use[ \t]+((?:crate|super|self|{IDENT})(?:::(?:super|self|{IDENT}))*)::\{{([^}}]*)\}}"
        ))
        .expect("static regex")
    })
}

/// A contiguous path: a keyword head, or an identifier chain with at least one
/// `::`. The retained lookaround (not preceded by a word character or `:`,
/// not followed by a word character) is checked around each candidate.
fn chain() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(&format!(
            r"(?:(?:crate|super|self)(?-u:\b)|{IDENT}::(?:super|self|{IDENT}))(?:::(?:super|self|{IDENT}))*"
        ))
        .expect("static regex")
    })
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn starts_with_segments(path: &[String], prefix: &[String]) -> bool {
    path.len() >= prefix.len() && path[..prefix.len()] == *prefix
}

fn split_chain(text: &str) -> Vec<String> {
    text.split("::").map(str::to_owned).collect()
}

/// Resolve a chain against module `m` (None when the file's module is unknown).
fn resolve_chain(
    chain: &[String],
    m: Option<&[String]>,
    declared: &BTreeSet<String>,
    bindings: &BTreeMap<String, Binding>,
) -> Option<Vec<String>> {
    let head = chain.first()?;
    if head == "crate" {
        return Some(
            chain[1..]
                .iter()
                .filter(|segment| *segment != "self")
                .cloned()
                .collect(),
        );
    }
    if head == "self" || head == "super" {
        let mut base = m?.to_vec();
        let mut index = 0;
        while index < chain.len() {
            match chain[index].as_str() {
                "super" => {
                    base.pop()?;
                }
                "self" => {}
                _ => break,
            }
            index += 1;
        }
        base.extend_from_slice(&chain[index..]);
        return Some(base);
    }
    if declared.contains(head) {
        let mut abs = m?.to_vec();
        abs.extend_from_slice(chain);
        return Some(abs);
    }
    let binding = bindings.get(head)?;
    let mut abs = binding.abs.clone();
    abs.extend_from_slice(&chain[1..]);
    Some(abs)
}

/// Members of a brace group as segment lists (`self` and nested groups skipped).
fn group_members(group: &str) -> Vec<(Vec<String>, Option<String>)> {
    static AS: OnceLock<Regex> = OnceLock::new();
    let as_split = AS.get_or_init(|| Regex::new(r"[ \t]+as[ \t]+").expect("static regex"));
    group
        .split(',')
        .filter_map(|raw| {
            let mut parts = as_split.splitn(raw.trim(), 2);
            let path_part = parts.next().unwrap_or("");
            let alias = parts.next().map(str::to_owned);
            let segs: Vec<String> = path_part
                .split("::")
                .map(|segment| segment.trim().to_owned())
                .filter(|segment| !segment.is_empty())
                .collect();
            if segs.is_empty()
                || segs
                    .iter()
                    .any(|segment| segment.contains('{') || segment == "*")
            {
                return None;
            }
            Some((segs, alias))
        })
        .collect()
}

fn collect_bindings(
    content: &str,
    m: Option<&[String]>,
    declared: &BTreeSet<String>,
) -> BTreeMap<String, Binding> {
    let empty = BTreeMap::new();
    let mut out = BTreeMap::new();
    for captures in use_single().captures_iter(content) {
        let chain = split_chain(&captures[1]);
        let Some(abs) = resolve_chain(&chain, m, declared, &empty) else {
            continue;
        };
        if abs.is_empty() {
            continue;
        }
        let alias = captures.get(2).map(|alias| alias.as_str().to_owned());
        let name = alias.clone().unwrap_or_else(|| {
            chain
                .last()
                .cloned()
                .expect("a chain has at least one segment")
        });
        out.insert(
            name,
            Binding {
                abs,
                aliased: alias.is_some(),
                via_group: false,
            },
        );
    }
    for captures in use_group().captures_iter(content) {
        let Some(prefix) = resolve_chain(&split_chain(&captures[1]), m, declared, &empty) else {
            continue;
        };
        for (segs, alias) in group_members(&captures[2]) {
            let is_self = segs.len() == 1 && segs[0] == "self";
            let abs = if is_self {
                prefix.clone()
            } else {
                let mut abs = prefix.clone();
                abs.extend(segs.iter().filter(|s| *s != "self").cloned());
                abs
            };
            let name = alias.clone().or_else(|| {
                if is_self {
                    prefix.last().cloned()
                } else {
                    segs.last().cloned()
                }
            });
            let Some(name) = name else {
                continue;
            };
            out.insert(
                name,
                Binding {
                    abs,
                    aliased: alias.is_some(),
                    via_group: true,
                },
            );
        }
    }
    out
}

fn absolute(abs: &[String]) -> String {
    if abs.is_empty() {
        "crate".to_owned()
    } else {
        format!("crate::{}", abs.join("::"))
    }
}

/// Re-emit `abs` from `emit_module` in the style of `chain`.
fn emit_path(
    chain: &[String],
    abs: &[String],
    ctx: &RewriteContext<'_>,
    bindings: &BTreeMap<String, Binding>,
) -> String {
    let head = chain[0].as_str();
    let Some(m) = ctx.emit_module else {
        return absolute(abs);
    };
    if head == "crate" {
        return absolute(abs);
    }
    if head == "self" || head == "super" {
        let mut base = m.to_vec();
        let mut prefix: Vec<&str> = Vec::new();
        for token in chain {
            match token.as_str() {
                "super" => {
                    if base.pop().is_none() {
                        return absolute(abs);
                    }
                    prefix.push("super");
                }
                "self" => prefix.push("self"),
                _ => break,
            }
        }
        return if starts_with_segments(abs, &base) {
            prefix
                .into_iter()
                .map(str::to_owned)
                .chain(abs[base.len()..].iter().cloned())
                .collect::<Vec<_>>()
                .join("::")
        } else {
            absolute(abs)
        };
    }
    if ctx.declared_before.contains(head) {
        // Resolved through a module declared in this file: same style if the
        // first segment below this module is (still) declared here.
        let next = abs.get(m.len());
        if starts_with_segments(abs, m)
            && next.is_some_and(|next| ctx.declared_after.contains(next))
        {
            return abs[m.len()..].join("::");
        }
        return absolute(abs);
    }
    if let Some(binding) = bindings.get(head).filter(|binding| !binding.via_group) {
        // The binding's own `use` line is rewritten by the chain rule, so its
        // local name becomes the new last segment unless it was aliased.
        let bound_abs: Vec<String> = if starts_with_segments(&binding.abs, ctx.old_module) {
            ctx.new_module
                .iter()
                .chain(&binding.abs[ctx.old_module.len()..])
                .cloned()
                .collect()
        } else {
            binding.abs.clone()
        };
        if starts_with_segments(abs, &bound_abs) {
            let name = if binding.aliased {
                head.to_owned()
            } else {
                bound_abs.last().cloned().unwrap_or_default()
            };
            return std::iter::once(name)
                .chain(abs[bound_abs.len()..].iter().cloned())
                .collect::<Vec<_>>()
                .join("::");
        }
    }
    absolute(abs)
}

/// Rewrite every contiguous path in `content` that resolves into the moved
/// module; for the moved file itself (resolve_module != emit_module) every
/// relative path is re-emitted from its new location. Returns the new
/// content, the rewrite count, and brace-group sites left alone.
pub(super) fn rewrite_paths(
    content: &str,
    ctx: &RewriteContext<'_>,
) -> (String, usize, Vec<(usize, String)>) {
    let bindings = collect_bindings(content, ctx.resolve_module, ctx.declared_before);
    let relocated = ctx.resolve_module != ctx.emit_module;
    let mut count = 0;
    let mut out = String::with_capacity(content.len());
    let mut cursor = 0;
    let mut search = 0;
    let bytes = content.as_bytes();
    while let Some(found) = chain().find_at(content, search) {
        let preceded = found.start() > 0 && {
            let previous = bytes[found.start() - 1];
            is_word(previous) || previous == b':'
        };
        let followed = bytes.get(found.end()).is_some_and(|byte| is_word(*byte));
        if preceded || followed {
            search = found.start() + 1;
            continue;
        }
        search = found.end();
        let whole = found.as_str();
        let chain = split_chain(whole);
        let Some(abs) = resolve_chain(&chain, ctx.resolve_module, ctx.declared_before, &bindings)
        else {
            continue;
        };
        let in_moved = starts_with_segments(&abs, ctx.old_module);
        if !in_moved && (!relocated || chain[0] == "crate") {
            continue;
        }
        let target: Vec<String> = if in_moved {
            ctx.new_module
                .iter()
                .chain(&abs[ctx.old_module.len()..])
                .cloned()
                .collect()
        } else {
            abs
        };
        let next = emit_path(&chain, &target, ctx, &bindings);
        if next == whole {
            continue;
        }
        out.push_str(&content[cursor..found.start()]);
        out.push_str(&next);
        cursor = found.end();
        count += 1;
    }
    out.push_str(&content[cursor..]);
    // Brace groups whose member names the moved module (when the prefix itself
    // is inside it, the chain rule already rewrote the prefix).
    let mut unrewritten = Vec::new();
    for captures in use_group().captures_iter(content) {
        let Some(prefix) = resolve_chain(
            &split_chain(&captures[1]),
            ctx.resolve_module,
            ctx.declared_before,
            &bindings,
        ) else {
            continue;
        };
        if starts_with_segments(&prefix, ctx.old_module) {
            continue;
        }
        let hit = group_members(&captures[2]).iter().any(|(segs, _)| {
            let full: Vec<String> = prefix
                .iter()
                .cloned()
                .chain(segs.iter().filter(|s| *s != "self").cloned())
                .collect();
            starts_with_segments(&full, ctx.old_module)
        });
        if !hit {
            continue;
        }
        let whole = captures.get(0).expect("whole match");
        let line = content[..whole.start()].matches('\n').count() + 1;
        unrewritten.push((line, whole.as_str().trim().to_owned()));
    }
    (out, count, unrewritten)
}

#[cfg(test)]
#[path = "rewrite.tests.rs"]
mod tests;
