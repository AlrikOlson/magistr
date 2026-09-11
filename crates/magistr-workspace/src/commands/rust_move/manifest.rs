use std::sync::OnceLock;

use regex::Regex;

use super::super::source_reads::SourceReads;

const TARGET_TABLES: [&str; 5] = ["[lib]", "[[bin]]", "[[test]]", "[[example]]", "[[bench]]"];

fn path_line() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?m)^([ \t]*path[ \t]*=[ \t]*")([^"\n]+)(")"#).expect("static regex")
    })
}

/// Split at lines that open a table; textual and section-based, no TOML parser.
fn sections(manifest: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in manifest.split_inclusive('\n') {
        let opens = line.trim_start_matches([' ', '\t']).starts_with('[');
        match out.last_mut() {
            Some(current) if !opens => current.push_str(line),
            _ => out.push(line.to_owned()),
        }
    }
    out
}

fn is_target_section(section: &str) -> bool {
    let header = section.trim_start();
    TARGET_TABLES.iter().any(|table| header.starts_with(table))
}

/// Crate-relative `path = "..."` values under the target tables.
pub(super) fn manifest_target_paths(manifest: &str) -> Vec<String> {
    sections(manifest)
        .iter()
        .filter(|section| is_target_section(section))
        .filter_map(|section| path_line().captures(section))
        .map(|captures| SourceReads::normalize(&captures[2]))
        .collect()
}

/// Rewrite target `path` entries equal to `from_rel` to `to_rel`; counts entries.
pub(super) fn rewrite_manifest_paths(
    manifest: &str,
    from_rel: &str,
    to_rel: &str,
) -> (String, usize) {
    let mut count = 0;
    let content = sections(manifest)
        .into_iter()
        .map(|section| {
            if !is_target_section(&section) {
                return section;
            }
            let mut replaced = false;
            path_line()
                .replacen(&section, 1, |captures: &regex::Captures<'_>| {
                    if replaced || SourceReads::normalize(&captures[2]) != from_rel {
                        return captures[0].to_owned();
                    }
                    replaced = true;
                    count += 1;
                    format!("{}{to_rel}{}", &captures[1], &captures[3])
                })
                .into_owned()
        })
        .collect();
    (content, count)
}

#[cfg(test)]
#[path = "manifest.tests.rs"]
mod tests;
