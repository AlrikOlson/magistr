//! The three core capabilities (SDD §3): parse (tree-sitter), search (the
//! grep crate — ripgrep's engine as a library), and rewrite (ast-grep
//! structural match/replace), plus the startup self-test that exercises all
//! three before the engine child reports ready.

use crate::langs::{WbLang, language_ids};
use ast_grep_core::matcher::Pattern;
use ast_grep_core::tree_sitter::LanguageExt;
use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::SearcherBuilder;
use grep::searcher::sinks::UTF8;
use thiserror::Error;
use tree_sitter::Parser;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("unknown language `{id}`; bundled languages: {known}")]
    UnknownLanguage { id: String, known: String },
    #[error("parser rejected language `{id}`: {detail}")]
    LanguageRejected { id: String, detail: String },
    #[error("parse returned no tree for language `{id}`")]
    ParseFailed { id: String },
    #[error("invalid search pattern: {detail}")]
    BadSearchPattern { detail: String },
    #[error("search failed: {detail}")]
    SearchFailed { detail: String },
    #[error("invalid rewrite pattern for `{id}`: {detail}")]
    BadRewritePattern { id: String, detail: String },
    #[error("invalid structural pattern for `{id}`: {detail}")]
    BadStructuralPattern { id: String, detail: String },
    #[error("rewrite produced non-UTF-8 output")]
    NonUtf8Output,
    #[error("workspace scan failed: {detail}")]
    ScanFailed { detail: String },
}

fn unknown_language(id: &str) -> CoreError {
    CoreError::UnknownLanguage {
        id: id.to_string(),
        known: language_ids().join(", "),
    }
}

/// Summary of one parse: enough to prove the grammar ran and report health.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSummary {
    pub language: String,
    pub root_kind: String,
    pub node_count: u32,
    pub error_count: u32,
    pub has_error: bool,
}

/// Parse `source` with the bundled grammar `lang_id`, or — after a bundled
/// miss, never before (SC-002) — a registered lang-pack grammar.
pub fn parse(lang_id: &str, source: &str) -> Result<ParseSummary, CoreError> {
    let Some(lang) = WbLang::get(lang_id) else {
        #[cfg(feature = "lang-pack")]
        if let Some(result) = crate::lang_registry::parse(lang_id, source) {
            return result;
        }
        return Err(unknown_language(lang_id));
    };
    let mut parser = Parser::new();
    parser
        .set_language(&lang.ts_language())
        .map_err(|e| CoreError::LanguageRejected {
            id: lang_id.to_string(),
            detail: e.to_string(),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CoreError::ParseFailed {
            id: lang_id.to_string(),
        })?;
    let root = tree.root_node();

    let mut error_count: u32 = 0;
    let mut cursor = root.walk();
    let mut descending = true;
    loop {
        if descending {
            let node = cursor.node();
            if node.is_error() || node.is_missing() {
                error_count += 1;
            }
        }
        if (descending && cursor.goto_first_child()) || cursor.goto_next_sibling() {
            descending = true;
        } else if cursor.goto_parent() {
            descending = false;
        } else {
            break;
        }
    }

    Ok(ParseSummary {
        language: lang.id().to_string(),
        root_kind: root.kind().to_string(),
        node_count: u32::try_from(root.descendant_count()).unwrap_or(u32::MAX),
        error_count,
        has_error: root.has_error(),
    })
}

/// One matching line from a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// 1-based line number.
    pub line: u64,
    /// Byte offset of the first match within the line.
    pub start: u32,
    /// Byte offset of the end of the first match within the line.
    pub end: u32,
    /// The matching line, without its trailing newline.
    pub line_text: String,
}

/// Regex-search `haystack`, returning every matching line.
pub fn search(
    pattern: &str,
    haystack: &str,
    case_insensitive: bool,
) -> Result<Vec<SearchHit>, CoreError> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(case_insensitive)
        .build(pattern)
        .map_err(|e| CoreError::BadSearchPattern {
            detail: e.to_string(),
        })?;
    let mut hits = Vec::new();
    let mut error: Option<String> = None;
    SearcherBuilder::new()
        .line_number(true)
        .build()
        .search_slice(
            &matcher,
            haystack.as_bytes(),
            UTF8(|line_number, line| {
                let trimmed = line.trim_end_matches(['\n', '\r']);
                match matcher.find(trimmed.as_bytes()) {
                    Ok(found) => {
                        let (start, end) = found
                            .map(|m| (m.start(), m.end()))
                            .unwrap_or((0, trimmed.len()));
                        hits.push(SearchHit {
                            line: line_number,
                            start: u32::try_from(start).unwrap_or(u32::MAX),
                            end: u32::try_from(end).unwrap_or(u32::MAX),
                            line_text: trimmed.to_string(),
                        });
                        Ok(true)
                    }
                    Err(e) => {
                        error = Some(e.to_string());
                        Ok(false)
                    }
                }
            }),
        )
        .map_err(|e| CoreError::SearchFailed {
            detail: e.to_string(),
        })?;
    if let Some(detail) = error {
        return Err(CoreError::SearchFailed { detail });
    }
    Ok(hits)
}

/// One structural match: an AST node matched by an ast-grep pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralHit {
    /// 1-based line of the match start.
    pub line: u64,
    /// 1-based byte column of the match start within its line.
    pub column: u32,
    /// 1-based line of the match end.
    pub end_line: u64,
    /// tree-sitter node kind of the matched node (e.g. `call_expression`).
    pub kind: String,
    /// The full source line containing the match start, without its newline.
    pub line_text: String,
}

/// Structurally match `pattern` against `source` (ast-grep semantics: `$VAR`
/// metavariables), returning every matching node with its position and kind.
pub fn structural_search(
    lang_id: &str,
    source: &str,
    pattern: &str,
) -> Result<Vec<StructuralHit>, CoreError> {
    let lang = WbLang::get(lang_id).ok_or_else(|| unknown_language(lang_id))?;
    let pattern = Pattern::try_new(pattern, lang).map_err(|e| CoreError::BadStructuralPattern {
        id: lang_id.to_string(),
        detail: e.to_string(),
    })?;
    // Byte offset of each line start, so hit positions come from the same
    // source string the caller sent (never the parser's own accounting).
    let mut line_starts: Vec<usize> = vec![0];
    for (offset, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(offset + 1);
        }
    }
    let line_of = |offset: usize| -> usize {
        line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    };
    let line_text_of = |line: usize| -> &str {
        let start = line_starts[line];
        let end = line_starts
            .get(line + 1)
            .map_or(source.len(), |&next| next - 1);
        source[start..end].trim_end_matches('\r')
    };

    let root = lang.ast_grep(source);
    let hits = root
        .root()
        .find_all(&pattern)
        .map(|m| {
            let node = m.get_node();
            let range = node.range();
            let start_line = line_of(range.start);
            let end_line = line_of(range.end.saturating_sub(1).max(range.start));
            StructuralHit {
                line: start_line as u64 + 1,
                column: u32::try_from(range.start - line_starts[start_line] + 1)
                    .unwrap_or(u32::MAX),
                end_line: end_line as u64 + 1,
                kind: node.kind().to_string(),
                line_text: line_text_of(start_line).to_string(),
            }
        })
        .collect();
    Ok(hits)
}

/// Outcome of a structural rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteOutcome {
    pub output: String,
    pub match_count: u32,
}

/// Structurally rewrite every match of `pattern` in `source` with `replacement`
/// (ast-grep semantics: `$VAR` metavariables carry across).
pub fn rewrite(
    lang_id: &str,
    source: &str,
    pattern: &str,
    replacement: &str,
) -> Result<RewriteOutcome, CoreError> {
    let lang = WbLang::get(lang_id).ok_or_else(|| unknown_language(lang_id))?;
    let pattern = Pattern::try_new(pattern, lang).map_err(|e| CoreError::BadRewritePattern {
        id: lang_id.to_string(),
        detail: e.to_string(),
    })?;
    let root = lang.ast_grep(source);
    let mut edits = root.root().replace_all(&pattern, replacement);
    edits.sort_by_key(|e| e.position);

    let mut out: Vec<u8> = Vec::with_capacity(source.len());
    let src = source.as_bytes();
    let mut cursor = 0usize;
    for edit in &edits {
        // Overlapping edits should not happen with reentrant(false); skip defensively.
        if edit.position < cursor {
            continue;
        }
        out.extend_from_slice(&src[cursor..edit.position]);
        out.extend_from_slice(&edit.inserted_text);
        cursor = edit.position + edit.deleted_length;
    }
    out.extend_from_slice(&src[cursor..]);

    Ok(RewriteOutcome {
        output: String::from_utf8(out).map_err(|_| CoreError::NonUtf8Output)?,
        match_count: u32::try_from(edits.len()).unwrap_or(u32::MAX),
    })
}

/// Result of the startup self-test (SDD §3): parse, search and rewrite must
/// all pass before the server reports ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTestReport {
    pub parse_ok: bool,
    pub search_ok: bool,
    pub rewrite_ok: bool,
    pub details: Vec<String>,
}

impl SelfTestReport {
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.parse_ok && self.search_ok && self.rewrite_ok
    }
}

/// Exercise parse, search and rewrite end to end on known inputs.
#[must_use]
pub fn self_test() -> SelfTestReport {
    let mut details = Vec::new();

    let parse_ok = match parse("typescript", "const answer: number = 42;\n") {
        Ok(summary) if !summary.has_error && summary.node_count > 1 => true,
        Ok(summary) => {
            details.push(format!(
                "parse: unexpected summary (has_error={}, node_count={})",
                summary.has_error, summary.node_count
            ));
            false
        }
        Err(e) => {
            details.push(format!("parse: {e}"));
            false
        }
    };

    let search_ok = match search(r"nee+dle", "hay\nthe needle here\nhay\n", false) {
        Ok(hits) if hits.len() == 1 && hits[0].line == 2 => true,
        Ok(hits) => {
            details.push(format!("search: expected 1 hit on line 2, got {hits:?}"));
            false
        }
        Err(e) => {
            details.push(format!("search: {e}"));
            false
        }
    };

    let rewrite_ok = match rewrite(
        "javascript",
        "console.log(a);\nconsole.log(b);\n",
        "console.log($A)",
        "console.debug($A)",
    ) {
        Ok(outcome)
            if outcome.match_count == 2
                && outcome.output == "console.debug(a);\nconsole.debug(b);\n" =>
        {
            true
        }
        Ok(outcome) => {
            details.push(format!(
                "rewrite: unexpected outcome (match_count={}, output={:?})",
                outcome.match_count, outcome.output
            ));
            false
        }
        Err(e) => {
            details.push(format!("rewrite: {e}"));
            false
        }
    };

    SelfTestReport {
        parse_ok,
        search_ok,
        rewrite_ok,
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reports_clean_typescript() {
        let s = parse("typescript", "export const x: string = \"hi\";\n").unwrap();
        assert_eq!(s.language, "typescript");
        assert_eq!(s.root_kind, "program");
        assert!(!s.has_error);
        assert_eq!(s.error_count, 0);
        assert!(s.node_count > 5);
    }

    #[test]
    fn parse_counts_errors_in_broken_source() {
        let s = parse("json", "{ \"a\": ").unwrap();
        assert!(s.has_error);
        assert!(s.error_count > 0);
    }

    #[test]
    fn parse_unknown_language_is_a_typed_error() {
        let e = parse("cobol", "x").unwrap_err();
        assert!(matches!(e, CoreError::UnknownLanguage { .. }));
        assert!(e.to_string().contains("typescript"));
    }

    #[test]
    fn parse_works_for_every_bundled_language() {
        // Not a grammar-fidelity test — proof each grammar links and runs.
        for (id, _) in crate::langs::LANGUAGES {
            let s = parse(id, "x").unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(s.node_count >= 1, "{id}");
        }
    }

    #[test]
    fn search_finds_lines_with_offsets() {
        let hits = search(
            r"fn \w+",
            "fn main() {}\nlet x = 1;\nfn helper() {}\n",
            false,
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 1);
        assert_eq!((hits[0].start, hits[0].end), (0, 7));
        assert_eq!(hits[1].line, 3);
        assert_eq!(hits[1].line_text, "fn helper() {}");
    }

    #[test]
    fn search_case_insensitive_flag() {
        assert!(search("needle", "NEEDLE\n", false).unwrap().is_empty());
        assert_eq!(search("needle", "NEEDLE\n", true).unwrap().len(), 1);
    }

    #[test]
    fn search_bad_pattern_is_a_typed_error() {
        assert!(matches!(
            search("(unclosed", "x", false).unwrap_err(),
            CoreError::BadSearchPattern { .. }
        ));
    }

    #[test]
    fn structural_finds_call_expressions_with_metavariables() {
        let hits = structural_search(
            "typescript",
            "const a = 1;\nconsole.log(a);\nkeep(a);\nconsole.log(b);\n",
            "console.log($A)",
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[0].column, 1);
        assert_eq!(hits[0].kind, "call_expression");
        assert_eq!(hits[0].line_text, "console.log(a);");
        assert_eq!(hits[1].line, 4);
    }

    #[test]
    fn structural_reports_multiline_end() {
        let hits = structural_search(
            "python",
            "def outer():\n    pass\n\ndef inner():\n    return 1\n",
            "def $NAME(): $$$BODY",
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].kind, "function_definition");
        assert_eq!((hits[0].line, hits[0].end_line), (1, 2));
        assert_eq!((hits[1].line, hits[1].end_line), (4, 5));
    }

    #[test]
    fn structural_works_where_dollar_is_not_an_identifier_char() {
        let hits = structural_search(
            "go",
            "package m\n\nfunc f() { fmt.Println(x) }\n",
            "fmt.Println($A)",
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "call_expression");
    }

    #[test]
    fn structural_bad_pattern_is_a_typed_error() {
        let e = structural_search("typescript", "const x = 1;\n", "").unwrap_err();
        assert!(matches!(e, CoreError::BadStructuralPattern { .. }));
        assert!(e.to_string().contains("invalid structural pattern"));
    }

    #[test]
    fn structural_with_no_matches_returns_empty() {
        let hits = structural_search("typescript", "const x = 1;\n", "console.log($A)").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn rewrite_carries_metavariables() {
        let out = rewrite(
            "javascript",
            "console.log(a);\nkeep(b);\nconsole.log(c);\n",
            "console.log($A)",
            "console.debug($A)",
        )
        .unwrap();
        assert_eq!(out.match_count, 2);
        assert_eq!(
            out.output,
            "console.debug(a);\nkeep(b);\nconsole.debug(c);\n"
        );
    }

    #[test]
    fn rewrite_works_where_dollar_is_not_an_identifier_char() {
        // python, go, c: `$A` must survive via the per-language expando char.
        let out = rewrite(
            "python",
            "print(1)\nkeep(2)\nprint(3)\n",
            "print($A)",
            "log($A)",
        )
        .unwrap();
        assert_eq!(out.match_count, 2);
        assert_eq!(out.output, "log(1)\nkeep(2)\nlog(3)\n");

        let out = rewrite(
            "go",
            "fmt.Println(x)\n",
            "fmt.Println($A)",
            "logger.Info($A)",
        )
        .unwrap();
        assert_eq!(out.match_count, 1);
        assert_eq!(out.output, "logger.Info(x)\n");

        // C: a bare call is grammatically ambiguous (macro type specifier), so
        // the idiomatic ast-grep form is the semicolon-terminated statement.
        let out = rewrite(
            "c",
            "int main() {\n  puts(msg);\n  return 0;\n}\n",
            "puts($A);",
            "log_line($A);",
        )
        .unwrap();
        assert_eq!(out.match_count, 1);
        assert_eq!(
            out.output,
            "int main() {\n  log_line(msg);\n  return 0;\n}\n"
        );
    }

    #[test]
    fn rewrite_with_no_matches_returns_source_unchanged() {
        let out = rewrite("python", "print(1)\n", "sleep($X)", "wait($X)").unwrap();
        assert_eq!(out.match_count, 0);
        assert_eq!(out.output, "print(1)\n");
    }

    #[test]
    fn self_test_is_green() {
        let report = self_test();
        assert!(report.all_ok(), "details: {:?}", report.details);
        assert!(report.details.is_empty());
    }
}
