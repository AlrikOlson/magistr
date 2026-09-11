//! The bundled grammar registry (SDD §3): twelve grammar crates, thirteen
//! language ids (`tree-sitter-typescript` provides both `typescript` and `tsx`).
//!
//! `WbLang` is the single language handle used by parsing (tree-sitter) and
//! structural rewrite (ast-grep). Grammar crates are decoupled from the
//! tree-sitter runtime version via `tree_sitter_language::LanguageFn`.

use std::borrow::Cow;

use ast_grep_core::language::Language;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use tree_sitter_language::LanguageFn;

/// Every bundled language, in id order.
pub const LANGUAGES: &[(&str, LanguageFn)] = &[
    ("bash", tree_sitter_bash::LANGUAGE),
    ("c", tree_sitter_c::LANGUAGE),
    ("cpp", tree_sitter_cpp::LANGUAGE),
    ("csharp", tree_sitter_c_sharp::LANGUAGE),
    ("go", tree_sitter_go::LANGUAGE),
    ("java", tree_sitter_java::LANGUAGE),
    ("javascript", tree_sitter_javascript::LANGUAGE),
    ("json", tree_sitter_json::LANGUAGE),
    ("python", tree_sitter_python::LANGUAGE),
    ("ruby", tree_sitter_ruby::LANGUAGE),
    ("rust", tree_sitter_rust::LANGUAGE),
    ("typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
    ("tsx", tree_sitter_typescript::LANGUAGE_TSX),
];

/// All registered language ids.
pub fn language_ids() -> Vec<&'static str> {
    LANGUAGES.iter().map(|(id, _)| *id).collect()
}

/// One bundled language, resolvable by id.
#[derive(Clone, Copy)]
pub struct WbLang {
    id: &'static str,
    lang: LanguageFn,
}

impl std::fmt::Debug for WbLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WbLang").field("id", &self.id).finish()
    }
}

impl WbLang {
    /// Resolve a language by its registry id.
    #[must_use]
    pub fn get(id: &str) -> Option<Self> {
        LANGUAGES
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(known, lang)| Self {
                id: known,
                lang: *lang,
            })
    }

    #[must_use]
    pub fn id(&self) -> &'static str {
        self.id
    }

    #[must_use]
    pub fn ts_language(&self) -> TSLanguage {
        self.lang.into()
    }
}

/// Rewrite `$`-sigils in a pattern to the language's expando char so the
/// pattern parses as a valid identifier. Same algorithm as ast-grep's own
/// language pack (Apache-2.0), which our bundled set mirrors.
fn pre_process_pattern(expando: char, query: &str) -> Cow<'_, str> {
    if expando == '$' {
        return Cow::Borrowed(query);
    }
    let mut ret = Vec::with_capacity(query.len());
    let mut dollar_count = 0;
    for c in query.chars() {
        if c == '$' {
            dollar_count += 1;
            continue;
        }
        let need_replace = matches!(c, 'A'..='Z' | '_') // $A or $$A or $$$A
            || dollar_count == 3; // anonymous multiple
        let sigil = if need_replace { expando } else { '$' };
        ret.extend(std::iter::repeat_n(sigil, dollar_count));
        dollar_count = 0;
        ret.push(c);
    }
    // trailing anonymous multiple
    let sigil = if dollar_count == 3 { expando } else { '$' };
    ret.extend(std::iter::repeat_n(sigil, dollar_count));
    Cow::Owned(ret.into_iter().collect())
}

impl Language for WbLang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.ts_language().id_for_node_kind(kind, /* named */ true)
    }

    /// Languages where `$` cannot start an identifier need a substitute char
    /// that can. Values mirror ast-grep's language pack.
    fn expando_char(&self) -> char {
        match self.id {
            "c" | "cpp" => '𐀀',
            "csharp" | "go" | "python" | "ruby" | "rust" => 'µ',
            _ => '$',
        }
    }

    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        pre_process_pattern(self.expando_char(), query)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.ts_language().field_id_for_name(field).map(|f| f.get())
    }

    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        // Parse the pattern as a complete line: at EOF some grammars pick a
        // different reading (tree-sitter-go resolves `fmt.Println(x)` to a
        // type conversion without the newline, a call expression with it).
        builder.build(|src| {
            if src.ends_with('\n') {
                StrDoc::try_new(src, *self)
            } else {
                StrDoc::try_new(&format!("{src}\n"), *self)
            }
        })
    }
}

impl LanguageExt for WbLang {
    fn get_ts_language(&self) -> TSLanguage {
        self.ts_language()
    }
}

#[cfg(test)]
mod tests {
    use super::{LANGUAGES, WbLang, language_ids};

    #[test]
    fn thirteen_ids_from_twelve_grammar_crates() {
        assert_eq!(LANGUAGES.len(), 13);
        assert!(language_ids().contains(&"typescript"));
        assert!(language_ids().contains(&"tsx"));
    }

    #[test]
    fn every_registered_language_loads() {
        for (id, _) in LANGUAGES {
            let lang = WbLang::get(id).expect("registered id resolves");
            // Materializing the TSLanguage exercises the grammar's ABI.
            assert!(lang.ts_language().abi_version() > 0, "{id}");
        }
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(WbLang::get("cobol").is_none());
    }
}
