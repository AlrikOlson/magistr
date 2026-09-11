use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

use regex::Regex;

use super::{SearchGroup, SearchHit, ranking_candidate::RankingCandidate};

/// First-party deterministic ranking, independent of workspace I/O and workers.
pub(super) struct Ranking;

impl Ranking {
    pub fn rank(candidates: Vec<RankingCandidate>, query: &str) -> Vec<SearchGroup> {
        let classifier = Classifier::new(query);
        let tokens: Vec<_> = query_tokens()
            .find_iter(&query.to_ascii_lowercase())
            .take(8)
            .map(|matched| matched.as_str().to_owned())
            .collect();
        let mut groups: Vec<_> = candidates
            .into_iter()
            .map(|candidate| {
                let file_class = class_of(&candidate);
                let total_hits = candidate.hits.len();
                let mut hits: Vec<_> = candidate
                    .hits
                    .into_iter()
                    .map(|mut hit| {
                        let class = classify(&hit, &classifier);
                        if candidate.test_ranges.iter().any(|range| {
                            hit.line >= range.start_line.into() && hit.line <= range.end_line.into()
                        }) {
                            hit.in_test_range = Some(true);
                        }
                        ClassifiedHit { hit, class }
                    })
                    .collect();
                order_hits(&mut hits);
                ClassifiedGroup {
                    file: candidate.file,
                    junk: candidate.junk,
                    file_class,
                    total_hits,
                    hits,
                }
            })
            .collect();
        dedup(&mut groups);
        order_groups(&mut groups, &tokens);
        groups
            .into_iter()
            .map(|group| SearchGroup {
                file: group.file,
                junk: group.junk,
                total_hits: group.total_hits,
                hits: group.hits.into_iter().take(20).map(|hit| hit.hit).collect(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindClass {
    Declaration,
    Call,
    Import,
    Comment,
    String,
    Other,
}

impl KindClass {
    fn weight(self) -> f64 {
        match self {
            Self::Declaration => 4.0,
            Self::Call => 2.0,
            Self::Import => 1.5,
            Self::Other => 1.0,
            Self::String => 0.75,
            Self::Comment => 0.5,
        }
    }
    #[cfg(test)]
    fn name(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Call => "call",
            Self::Import => "import",
            Self::Comment => "comment",
            Self::String => "string",
            Self::Other => "other",
        }
    }
}

struct ClassifiedHit {
    hit: SearchHit,
    class: KindClass,
}

struct ClassifiedGroup {
    file: String,
    junk: bool,
    file_class: u8,
    total_hits: usize,
    hits: Vec<ClassifiedHit>,
}

fn class_of(group: &RankingCandidate) -> u8 {
    if group.junk {
        return 0;
    }
    if group.test
        || !group.hits.is_empty()
            && group.hits.iter().all(|hit| {
                group.test_ranges.iter().any(|range| {
                    hit.line >= range.start_line.into() && hit.line <= range.end_line.into()
                })
            })
    {
        return 1;
    }
    2
}

fn classify(hit: &SearchHit, classifier: &Classifier) -> KindClass {
    if let Some(kind) = hit.kind.as_deref().filter(|kind| *kind != "use") {
        if kind.contains("comment") {
            return KindClass::Comment;
        }
        if ["string", "str_lit", "template"]
            .iter()
            .any(|tag| kind.contains(tag))
        {
            return KindClass::String;
        }
        if ["import", "include", "require"]
            .iter()
            .any(|tag| kind.contains(tag))
        {
            return KindClass::Import;
        }
        if ["call", "invocation", "new_expression"]
            .iter()
            .any(|tag| kind.contains(tag))
        {
            return KindClass::Call;
        }
        if kind.contains("defin")
            || kind.contains("declar")
            || matches!(
                kind,
                "function"
                    | "method"
                    | "class"
                    | "struct"
                    | "enum"
                    | "interface"
                    | "trait"
                    | "type"
                    | "type_alias"
                    | "module"
                    | "constant"
                    | "variable"
                    | "field"
                    | "property"
                    | "macro"
            )
        {
            return KindClass::Declaration;
        }
    }
    let trimmed = hit.excerpt.trim_start();
    if ["//", "/*", "*", "--", ";;"]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || trimmed.starts_with('#') && !trimmed.starts_with("#[") && !trimmed.starts_with("#!")
    {
        return KindClass::Comment;
    }
    static IMPORT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\s*(?:import\b|from\s+\S+\s+import\b|export\s+.*\bfrom\b|use\s+[A-Za-z0-9_:{]|require\s*\(|#include\b|using\s+\w)").expect("constant import heuristic")
    });
    if IMPORT.is_match(&hit.excerpt) {
        return KindClass::Import;
    }
    if classifier
        .declaration
        .as_ref()
        .is_some_and(|regex| regex.is_match(&hit.excerpt))
    {
        return KindClass::Declaration;
    }
    if inside_string(&hit.excerpt, hit.column) {
        return KindClass::String;
    }
    if classifier
        .call
        .as_ref()
        .is_some_and(|regex| regex.is_match(&hit.excerpt))
    {
        return KindClass::Call;
    }
    KindClass::Other
}

fn is_identifier(query: &str) -> bool {
    !query.is_empty()
        && query
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
}

struct Classifier {
    declaration: Option<Regex>,
    call: Option<Regex>,
}

impl Classifier {
    fn new(query: &str) -> Self {
        Self {
            declaration: declaration_pattern(query),
            call: is_identifier(query)
                .then(|| Regex::new(&format!(r"(?-u:\b){}\s*\(", regex::escape(query))).ok())
                .flatten(),
        }
    }
}

fn declaration_pattern(query: &str) -> Option<Regex> {
    let declaration = "function|def|fn|class|struct|enum|trait|interface|impl|type|mod|module";
    let assignment = "const|let|var|val|static|pub";
    let name = if is_identifier(query) {
        regex::escape(query)
    } else {
        "[A-Za-z0-9_$]+".into()
    };
    let tail = if is_identifier(query) {
        r"(?-u:\b)"
    } else {
        ""
    };
    Regex::new(&format!(
        r"(?-u:\b)(?:{declaration})\s+{name}{tail}|(?-u:\b)(?:{assignment})\s+{name}\s*[=:(]"
    ))
    .ok()
}

fn inside_string(excerpt: &str, column: u32) -> bool {
    let mut quote = None;
    let mut previous = 0;
    for character in excerpt
        .encode_utf16()
        .take(column.saturating_sub(1) as usize)
    {
        if quote.is_none() && [b'"' as u16, b'\'' as u16, b'`' as u16].contains(&character) {
            quote = Some(character);
        } else if quote == Some(character) && previous != b'\\' as u16 {
            quote = None;
        }
        previous = character;
    }
    quote.is_some()
}

fn confidence(hit: &SearchHit) -> f64 {
    hit.confidence.map_or(1.0, super::SearchConfidence::weight)
}

fn order_hits(hits: &mut [ClassifiedHit]) {
    hits.sort_by(|a, b| {
        (b.class == KindClass::Declaration)
            .cmp(&(a.class == KindClass::Declaration))
            .then_with(|| confidence(&b.hit).total_cmp(&confidence(&a.hit)))
            .then_with(|| {
                a.hit
                    .in_test_range
                    .unwrap_or(false)
                    .cmp(&b.hit.in_test_range.unwrap_or(false))
            })
            .then_with(|| b.class.weight().total_cmp(&a.class.weight()))
            .then_with(|| a.hit.line.cmp(&b.hit.line))
    });
}

fn dedup(groups: &mut [ClassifiedGroup]) {
    let mut lines = BTreeMap::<String, (usize, usize)>::new();
    for (index, group) in groups.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for hit in &group.hits {
            let line = hit.hit.excerpt.trim();
            if line.encode_utf16().count() < 4 || !seen.insert(line) {
                continue;
            }
            let (count, canonical) = lines.entry(line.into()).or_insert((0, index));
            *count += 1;
            let prior = &groups[*canonical];
            if group.file_class > prior.file_class
                || group.file_class == prior.file_class && group.file < prior.file
            {
                *canonical = index;
            }
        }
    }
    for (index, group) in groups.iter_mut().enumerate() {
        group.hits.retain_mut(|hit| {
            let Some(&(count, canonical)) = lines.get(hit.hit.excerpt.trim()) else {
                return true;
            };
            if count < 3 {
                return true;
            }
            if canonical != index {
                return false;
            }
            hit.hit.duplicates = Some(count - 1);
            true
        });
    }
}

fn query_tokens() -> &'static Regex {
    static TOKENS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("[a-z0-9_]{3,}").expect("constant query token heuristic"));
    &TOKENS
}

fn order_groups(groups: &mut [ClassifiedGroup], tokens: &[String]) {
    let score = |group: &ClassifiedGroup| {
        let best = group
            .hits
            .iter()
            .map(|hit| hit.class.weight() * confidence(&hit.hit))
            .fold(0.0, f64::max);
        let path = group.file.to_lowercase();
        best + tokens
            .iter()
            .filter(|token| path.contains(token.as_str()))
            .count() as f64
            - 0.1 * group.file.split('/').count() as f64
    };
    let has_definition = |group: &ClassifiedGroup| {
        group
            .hits
            .iter()
            .any(|hit| hit.class == KindClass::Declaration)
    };
    let best_confidence = |group: &ClassifiedGroup| {
        group
            .hits
            .iter()
            .filter_map(|hit| hit.hit.confidence)
            .map(super::SearchConfidence::weight)
            .fold(0.0, f64::max)
    };
    groups.sort_by(|a, b| {
        has_definition(b)
            .cmp(&has_definition(a))
            .then_with(|| best_confidence(b).total_cmp(&best_confidence(a)))
            .then_with(|| b.file_class.cmp(&a.file_class))
            .then_with(|| score(b).total_cmp(&score(a)))
            .then_with(|| b.total_hits.cmp(&a.total_hits))
            .then_with(|| a.file.cmp(&b.file))
    });
}

#[cfg(test)]
#[path = "ranking.tests.rs"]
mod tests;
