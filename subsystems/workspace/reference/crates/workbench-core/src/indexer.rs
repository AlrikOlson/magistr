//! Tier-1 extraction and the workspace scan (SDD §7.3).
//!
//! `extract` turns one source file into its symbols and imports via the
//! bundled tree-sitter grammars; `scan_workspace` walks a workspace root with
//! gitignore semantics (the `ignore` crate — ripgrep's own walker) and flags
//! vendor/generated junk for ranking without excluding it. Both are plain
//! Rust; the napi layer exposes them as async tasks so the engine child's
//! frame loop stays responsive while workers parse in parallel.

use std::time::UNIX_EPOCH;

use tree_sitter::{Node, Parser};

use crate::engine::CoreError;
use crate::langs::WbLang;

/// One extracted symbol. Lines and columns are 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    /// Qualification the address dialect does not carry — today, the trait a
    /// Rust impl member came from, so `Type::member` stays the typeable
    /// address while an ambiguity can still say `<Type as Trait>::member`
    /// (specs/009 R2). `None` for everything else.
    pub detail: Option<String>,
    /// First line of the item's doc comment, trimmed and bounded (specs/009
    /// R6). `None` when the item carries no doc comment — absent is absent.
    pub doc: Option<String>,
}

/// Everything Tier 1 stores for one file: symbols, import targets, and the
/// line ranges of test-only regions (Rust `#[cfg(test)]` modules, specs/009
/// R4) whose hits rank as test code even though the file is source.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extraction {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<String>,
    pub test_ranges: Vec<TestRange>,
}

/// One inclusive, 1-based line range that holds test-only code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestRange {
    pub start_line: u32,
    pub end_line: u32,
}

/// Normalized symbol kind for a node kind in one language, if it declares one.
fn symbol_kind(lang: &str, node_kind: &str) -> Option<&'static str> {
    let kind = match (lang, node_kind) {
        ("bash", "function_definition") => "function",
        ("c" | "cpp", "function_definition") => "function",
        ("c" | "cpp", "struct_specifier" | "union_specifier") => "struct",
        ("c" | "cpp", "enum_specifier") => "enum",
        ("c" | "cpp", "type_definition") => "type",
        ("cpp", "class_specifier") => "class",
        ("cpp", "namespace_definition") => "namespace",
        ("csharp", "class_declaration" | "record_declaration") => "class",
        ("csharp", "interface_declaration") => "interface",
        ("csharp", "struct_declaration") => "struct",
        ("csharp", "enum_declaration") => "enum",
        ("csharp", "method_declaration" | "constructor_declaration") => "method",
        ("csharp", "namespace_declaration" | "file_scoped_namespace_declaration") => "namespace",
        ("go", "function_declaration") => "function",
        ("go", "method_declaration") => "method",
        ("go", "type_spec") => "type",
        ("java", "class_declaration" | "record_declaration") => "class",
        ("java", "interface_declaration") => "interface",
        ("java", "enum_declaration") => "enum",
        ("java", "method_declaration" | "constructor_declaration") => "method",
        ("javascript" | "typescript" | "tsx", "function_declaration") => "function",
        ("javascript" | "typescript" | "tsx", "generator_function_declaration") => "function",
        ("javascript" | "typescript" | "tsx", "class_declaration") => "class",
        ("javascript" | "typescript" | "tsx", "method_definition") => "method",
        ("typescript" | "tsx", "interface_declaration") => "interface",
        ("typescript" | "tsx", "type_alias_declaration") => "type",
        ("typescript" | "tsx", "enum_declaration") => "enum",
        ("typescript" | "tsx", "abstract_class_declaration") => "class",
        ("python", "function_definition") => "function",
        ("python", "class_definition") => "class",
        ("ruby", "method" | "singleton_method") => "method",
        ("ruby", "class") => "class",
        ("ruby", "module") => "module",
        ("rust", "function_item") => "function",
        ("rust", "struct_item") => "struct",
        ("rust", "enum_item") => "enum",
        ("rust", "trait_item") => "trait",
        ("rust", "mod_item") => "module",
        ("rust", "const_item" | "static_item") => "const",
        ("rust", "type_item") => "type",
        ("rust", "macro_definition") => "macro",
        _ => return None,
    };
    Some(kind)
}

fn text_of(node: Node, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Strip one layer of matching string delimiters: quotes or `<...>`.
pub(crate) fn strip_delimiters(raw: &str) -> String {
    let t = raw.trim();
    for (open, close) in [("\"", "\""), ("'", "'"), ("`", "`"), ("<", ">")] {
        if t.len() >= 2 && t.starts_with(open) && t.ends_with(close) {
            return t[open.len()..t.len() - close.len()].to_string();
        }
    }
    t.to_string()
}

/// Descend a C/C++ declarator chain to the declared identifier.
fn declarator_name(node: Node, source: &str) -> Option<String> {
    let mut current = node.child_by_field_name("declarator")?;
    loop {
        match current.kind() {
            "identifier"
            | "field_identifier"
            | "type_identifier"
            | "qualified_identifier"
            | "operator_name"
            | "destructor_name" => return text_of(current, source),
            "function_declarator"
            | "pointer_declarator"
            | "array_declarator"
            | "parenthesized_declarator"
            | "reference_declarator"
            | "init_declarator" => {
                current = current
                    .child_by_field_name("declarator")
                    .or_else(|| current.named_child(0))?;
            }
            _ => return None,
        }
    }
}

/// The declared name for a symbol-bearing node, when one is recoverable.
fn symbol_name(lang: &str, node: Node, source: &str) -> Option<String> {
    match (lang, node.kind()) {
        ("c" | "cpp", "function_definition" | "type_definition") => declarator_name(node, source),
        _ => node
            .child_by_field_name("name")
            .and_then(|n| text_of(n, source)),
    }
}

/// C/C++ `struct`/`enum`/`class` specifiers double as mere references; only a
/// specifier that carries a body declares something.
fn requires_body(node_kind: &str) -> bool {
    matches!(
        node_kind,
        "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier"
    )
}

/// A `const f = () => …` / `const f = function …` declarator is a function.
fn js_function_declarator(lang: &str, node: Node, source: &str) -> Option<Symbol> {
    if !matches!(lang, "javascript" | "typescript" | "tsx") {
        return None;
    }
    if node.kind() != "variable_declarator" {
        return None;
    }
    let value = node.child_by_field_name("value")?;
    if !matches!(
        value.kind(),
        "arrow_function" | "function_expression" | "function" | "generator_function"
    ) {
        return None;
    }
    let name = node
        .child_by_field_name("name")
        .and_then(|n| text_of(n, source))?;
    Some(symbol_at(name, "function", node))
}

fn symbol_at(name: String, kind: &'static str, node: Node) -> Symbol {
    let start = node.start_position();
    let end = node.end_position();
    Symbol {
        name,
        kind,
        start_line: u32::try_from(start.row + 1).unwrap_or(u32::MAX),
        start_col: u32::try_from(start.column + 1).unwrap_or(u32::MAX),
        end_line: u32::try_from(end.row + 1).unwrap_or(u32::MAX),
        detail: None,
        doc: None,
    }
}

/// Cap on a stored doc summary (specs/009 data-model): the outline is a
/// skeleton, and the full comment is one `read` away.
const DOC_SUMMARY_CAP: usize = 200;

/// The first line of the doc comment attached to `node`, if any.
///
/// Doc comments sit as sibling `line_comment`/`block_comment` nodes directly
/// above the item (tree-sitter-rust keeps them outside the item, unlike an
/// attribute). Walk backwards over contiguous comment siblings, take the
/// topmost `///`/`/** */` run's first line, strip the markers, and bound it.
/// Plain `//` comments are NOT doc comments and are ignored.
fn doc_summary(node: Node, source: &str) -> Option<String> {
    let mut first: Option<String> = None;
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        let kind = sibling.kind();
        if kind != "line_comment" && kind != "block_comment" {
            break;
        }
        let text = sibling.utf8_text(source.as_bytes()).ok()?;
        let Some(body) = doc_comment_body(text) else {
            break;
        };
        if let Some(line) = body.lines().map(str::trim).find(|l| !l.is_empty()) {
            first = Some(bound_summary(line));
        }
        current = sibling.prev_sibling();
    }
    first
}

/// The documentation body of a comment, or `None` when it is not a doc
/// comment (`//`, `/* */`) or is an inner doc (`//!`, `/*! */`, which
/// documents the enclosing module, not the next item).
fn doc_comment_body(text: &str) -> Option<&str> {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("///") {
        return Some(rest);
    }
    if let Some(rest) = t.strip_prefix("/**") {
        return rest.strip_suffix("*/").or(Some(rest));
    }
    None
}

/// Trim decoration and bound the length of one summary line.
fn bound_summary(line: &str) -> String {
    let trimmed = line.trim_start_matches('*').trim();
    if trimmed.chars().count() <= DOC_SUMMARY_CAP {
        return trimmed.to_string();
    }
    trimmed.chars().take(DOC_SUMMARY_CAP).collect()
}

/// The implementing type of a Rust `impl_item`, reduced to its base name so
/// the address stays typeable: `Stack<T>` qualifies as `Stack`.
fn impl_type_name(node: Node, source: &str) -> Option<String> {
    let ty = node.child_by_field_name("type")?;
    let base = match ty.kind() {
        "generic_type" => ty.child_by_field_name("type").unwrap_or(ty),
        _ => ty,
    };
    let text = text_of(base, source)?;
    Some(text.split('<').next().unwrap_or(&text).trim().to_string())
}

/// The trait a Rust `impl Trait for Type` block implements, base name only.
fn impl_trait_name(node: Node, source: &str) -> Option<String> {
    let tr = node.child_by_field_name("trait")?;
    let text = text_of(tr, source)?;
    Some(text.split('<').next().unwrap_or(&text).trim().to_string())
}

/// Members of a Rust `impl` block or `trait` declaration, qualified by the
/// type (or trait) that owns them (specs/009 R1/R2). Emitted here rather than
/// by the generic walk so the owner is in scope; the walk skips them to avoid
/// a bare duplicate. A trait's own method signatures are members too — they
/// are `function_signature_item`, which no kinds-table arm ever matched, so
/// before this they were invisible to the index entirely.
fn rust_qualified_members(node: Node, source: &str, out: &mut Vec<Symbol>) {
    let (owner, trait_name) = match node.kind() {
        "impl_item" => {
            let Some(type_name) = impl_type_name(node, source) else {
                return;
            };
            (type_name, impl_trait_name(node, source))
        }
        // `trait Greeter { fn hello(); }` — the trait owns the name, and there
        // is no separate trait to record: it IS the trait.
        _ => {
            let Some(name) = node
                .child_by_field_name("name")
                .and_then(|n| text_of(n, source))
            else {
                return;
            };
            (name, None)
        }
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        let kind = match member.kind() {
            "function_item" | "function_signature_item" => "method",
            "const_item" => "const",
            "type_item" | "associated_type" => "type",
            _ => continue,
        };
        let Some(name) = member
            .child_by_field_name("name")
            .and_then(|n| text_of(n, source))
        else {
            continue;
        };
        let mut symbol = symbol_at(format!("{owner}::{name}"), kind, member);
        symbol.detail = trait_name.clone();
        symbol.doc = doc_summary(member, source);
        out.push(symbol);
    }
}

/// Is this Rust `mod_item` gated on `#[cfg(test)]` (specs/009 R4)? Only
/// `cfg(test)` counts — `cfg(feature = "x")` and friends are ordinary code.
fn is_cfg_test_module(node: Node, source: &str) -> bool {
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        match sibling.kind() {
            "attribute_item" => {
                if let Ok(text) = sibling.utf8_text(source.as_bytes()) {
                    let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if squeezed == "#[cfg(test)]" {
                        return true;
                    }
                }
            }
            "line_comment" | "block_comment" => {}
            _ => break,
        }
        current = sibling.prev_sibling();
    }
    false
}

/// Record the import(s) a node declares, in one language, if any.
fn collect_imports(lang: &str, node: Node, source: &str, out: &mut Vec<String>) {
    match (lang, node.kind()) {
        ("rust", "use_declaration") => {
            if let Some(t) = node
                .child_by_field_name("argument")
                .and_then(|n| text_of(n, source))
            {
                out.push(t);
            }
        }
        ("python", "import_statement") => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let target = match child.kind() {
                    "dotted_name" => text_of(child, source),
                    "aliased_import" => child
                        .child_by_field_name("name")
                        .and_then(|n| text_of(n, source)),
                    _ => None,
                };
                if let Some(t) = target {
                    out.push(t);
                }
            }
        }
        ("python", "import_from_statement") => {
            if let Some(t) = node
                .child_by_field_name("module_name")
                .and_then(|n| text_of(n, source))
            {
                out.push(t);
            }
        }
        ("javascript" | "typescript" | "tsx", "import_statement" | "export_statement") => {
            if let Some(t) = node
                .child_by_field_name("source")
                .and_then(|n| text_of(n, source))
            {
                out.push(strip_delimiters(&t));
            }
        }
        ("go", "import_spec") => {
            if let Some(t) = node
                .child_by_field_name("path")
                .and_then(|n| text_of(n, source))
            {
                out.push(strip_delimiters(&t));
            }
        }
        ("java", "import_declaration") => {
            let mut cursor = node.walk();
            if let Some(t) = node
                .named_children(&mut cursor)
                .find(|c| matches!(c.kind(), "scoped_identifier" | "identifier"))
                .and_then(|n| text_of(n, source))
            {
                out.push(t);
            }
        }
        ("c" | "cpp", "preproc_include") => {
            if let Some(t) = node
                .child_by_field_name("path")
                .and_then(|n| text_of(n, source))
            {
                out.push(strip_delimiters(&t));
            }
        }
        ("csharp", "using_directive") => {
            let mut cursor = node.walk();
            let named = node.child_by_field_name("name").or_else(|| {
                node.named_children(&mut cursor)
                    .find(|c| matches!(c.kind(), "qualified_name" | "identifier"))
            });
            if let Some(t) = named.and_then(|n| text_of(n, source)) {
                out.push(t);
            }
        }
        ("ruby", "call") => {
            let is_require = node
                .child_by_field_name("method")
                .and_then(|n| text_of(n, source))
                .is_some_and(|m| m == "require" || m == "require_relative");
            if is_require
                && let Some(args) = node.child_by_field_name("arguments")
                && let Some(first) = args.named_child(0)
                && let Some(t) = text_of(first, source)
            {
                out.push(strip_delimiters(&t));
            }
        }
        _ => {}
    }
}

/// Extract symbols and imports from `source` with the bundled grammar
/// `lang_id`. Parse errors do not fail extraction — tree-sitter recovers, and
/// a partially broken file still yields its intact symbols.
pub fn extract(lang_id: &str, source: &str) -> Result<Extraction, CoreError> {
    let Some(lang) = WbLang::get(lang_id) else {
        // Bundled miss: a registered lang pack may own this id — its symbols
        // come from the pack's tag queries, never these static tables (R4).
        #[cfg(feature = "lang-pack")]
        if let Some(result) = crate::lang_registry::extract(lang_id, source) {
            return result;
        }
        return Err(CoreError::UnknownLanguage {
            id: lang_id.to_string(),
            known: crate::langs::language_ids().join(", "),
        });
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

    let id = lang.id();
    let mut extraction = Extraction::default();
    let mut cursor = tree.root_node().walk();
    let mut descending = true;
    loop {
        if descending {
            let node = cursor.node();
            visit(id, node, source, &mut extraction);
        }
        if (descending && cursor.goto_first_child()) || cursor.goto_next_sibling() {
            descending = true;
        } else if cursor.goto_parent() {
            descending = false;
        } else {
            break;
        }
    }
    Ok(extraction)
}

fn visit(lang: &str, node: Node, source: &str, out: &mut Extraction) {
    if lang == "rust" {
        match node.kind() {
            // Members are emitted qualified by the type (or trait) that owns
            // them (specs/009 R1); the generic arm below would emit them bare
            // as well, so these bodies are handled only here. The trait node
            // itself still reaches the generic arm and keeps its own symbol.
            "impl_item" => rust_qualified_members(node, source, &mut out.symbols),
            "trait_item" => rust_qualified_members(node, source, &mut out.symbols),
            "mod_item" if is_cfg_test_module(node, source) => {
                out.test_ranges.push(TestRange {
                    start_line: u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX),
                    end_line: u32::try_from(node.end_position().row + 1).unwrap_or(u32::MAX),
                });
            }
            _ => {}
        }
        if is_inside_impl(node) {
            collect_imports(lang, node, source, &mut out.imports);
            return;
        }
    }
    if let Some(kind) = symbol_kind(lang, node.kind()) {
        let declares = !requires_body(node.kind()) || node.child_by_field_name("body").is_some();
        if declares && let Some(name) = symbol_name(lang, node, source) {
            let mut symbol = symbol_at(name, kind, node);
            symbol.doc = doc_summary(node, source);
            out.symbols.push(symbol);
        }
    } else if let Some(symbol) = js_function_declarator(lang, node, source) {
        out.symbols.push(symbol);
    }
    collect_imports(lang, node, source, &mut out.imports);
}

/// Is this node a direct member of a Rust `impl` or `trait` body? Such
/// members are emitted qualified by [`rust_qualified_members`]; the generic
/// symbol arm must skip them so one method never yields both `Type::method`
/// and `method`.
fn is_inside_impl(node: Node) -> bool {
    node.parent()
        .filter(|p| p.kind() == "declaration_list")
        .and_then(|p| p.parent())
        .is_some_and(|g| matches!(g.kind(), "impl_item" | "trait_item"))
}

/// One file found by the workspace scan. `path` is workspace-relative with
/// `/` separators; `junk` marks vendor/generated content for ranking demotion
/// (never exclusion — SDD §7.3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    pub path: String,
    pub size: u64,
    pub mtime_ms: i64,
    pub junk: bool,
}

/// Directory components that mark vendor/generated content.
const JUNK_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "third_party",
    "dist",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".next",
    "coverage",
];

/// Lockfiles and build products: indexed, demoted in ranking.
const JUNK_FILES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lock",
    "bun.lockb",
    "Cargo.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
    "poetry.lock",
    "uv.lock",
];

const JUNK_SUFFIXES: &[&str] = &[".min.js", ".min.css", ".map", ".snap"];

/// Vendor/generated detection over a workspace-relative `/`-separated path.
#[must_use]
pub fn is_junk(rel_path: &str) -> bool {
    let mut components = rel_path.split('/').peekable();
    while let Some(component) = components.next() {
        let is_last = components.peek().is_none();
        if !is_last && JUNK_DIRS.contains(&component) {
            return true;
        }
        if is_last {
            return JUNK_FILES.contains(&component)
                || JUNK_SUFFIXES.iter().any(|s| component.ends_with(s));
        }
    }
    false
}

/// Directories that are never content: VCS internals and our own state.
const EXCLUDED_DIRS: &[&str] = &[".git", ".workbench"];

/// Walk `root` with gitignore semantics and junk flagging. Dotfiles are
/// walked (agent-facing corpora care about `.github/`, `.claude/`, …);
/// `.git/` and `.workbench/` are never content. Results are path-sorted so a
/// scan is deterministic for a given tree.
pub fn scan_workspace(root: &str) -> Result<Vec<ScanEntry>, CoreError> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !(entry.file_type().is_some_and(|t| t.is_dir())
                && EXCLUDED_DIRS.contains(&name.as_ref()))
        })
        .build();

    let mut entries = Vec::new();
    for result in walker {
        let entry = result.map_err(|e| CoreError::ScanFailed {
            detail: e.to_string(),
        })?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let metadata = entry.metadata().map_err(|e| CoreError::ScanFailed {
            detail: e.to_string(),
        })?;
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let mtime_ms = metadata
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_millis()).ok())
            .unwrap_or(0);
        entries.push(ScanEntry {
            junk: is_junk(&rel),
            path: rel,
            size: metadata.len(),
            mtime_ms,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(extraction: &Extraction) -> Vec<(&str, &str)> {
        extraction
            .symbols
            .iter()
            .map(|s| (s.name.as_str(), s.kind))
            .collect()
    }

    // ---- Rust index fidelity (specs/009-rust-index-fidelity) --------------

    /// A crate exercising every shape the chunk names: an inherent impl, a
    /// trait impl whose method collides with it, a generic impl, associated
    /// const/type, doc comments (documented and not), a `#[cfg(test)]` module
    /// and a `#[cfg(feature)]` module that must NOT count as test.
    const RUST_FIXTURE: &str = r#"use std::fmt::Debug;

/// Loads configuration.
/// Second line is ignored.
pub struct Config {
    pub path: String,
}

impl Config {
    /// Reads the config from disk.
    pub fn load() -> Config {
        Config { path: String::new() }
    }

    /// The default path.
    pub const DEFAULT: &'static str = "cfg.toml";

    pub type Handle = u32;

    pub fn hello(&self) -> u32 {
        1
    }
}

pub trait Greeter {
    fn hello(&self) -> u32;
}

impl Greeter for Config {
    // not a doc comment
    fn hello(&self) -> u32 {
        2
    }
}

pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    pub fn push(&mut self, _item: T) {}
}

#[cfg(feature = "extra")]
mod extra {
    pub fn helper() -> u32 {
        3
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(1, 1);
    }
}
"#;

    fn find<'a>(e: &'a Extraction, name: &str) -> &'a Symbol {
        e.symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no symbol {name:?} in {:?}", names(e)))
    }

    #[test]
    fn rust_impl_members_are_qualified_by_their_type() {
        let e = extract("rust", RUST_FIXTURE).unwrap();
        let qualified = names(&e);
        // Inherent members carry the type, with their own kinds.
        assert!(
            qualified.contains(&("Config::load", "method")),
            "{qualified:?}"
        );
        assert!(
            qualified.contains(&("Config::DEFAULT", "const")),
            "{qualified:?}"
        );
        assert!(
            qualified.contains(&("Config::Handle", "type")),
            "{qualified:?}"
        );
        // A generic impl qualifies by the BASE type name, so the address stays typeable.
        assert!(
            qualified.contains(&("Stack::push", "method")),
            "{qualified:?}"
        );
        // No member is ALSO emitted bare — one method, one symbol.
        assert!(
            !qualified.iter().any(|(n, _)| *n == "load"),
            "{qualified:?}"
        );
        assert!(
            !qualified.iter().any(|(n, _)| *n == "push"),
            "{qualified:?}"
        );
        // Free items keep their bare names and kinds.
        assert!(qualified.contains(&("Config", "struct")), "{qualified:?}");
        assert!(qualified.contains(&("Greeter", "trait")), "{qualified:?}");
    }

    #[test]
    fn a_trait_impl_member_keeps_the_trait_in_detail() {
        let e = extract("rust", RUST_FIXTURE).unwrap();
        // `Config::hello` exists twice — inherent and via Greeter. That is a
        // REAL ambiguity; `detail` is what lets an answer distinguish them.
        let hellos: Vec<&Symbol> = e
            .symbols
            .iter()
            .filter(|s| s.name == "Config::hello")
            .collect();
        assert_eq!(hellos.len(), 2, "{:?}", names(&e));
        assert!(
            hellos.iter().any(|s| s.detail.is_none()),
            "inherent impl carries no trait"
        );
        assert!(
            hellos
                .iter()
                .any(|s| s.detail.as_deref() == Some("Greeter")),
            "trait impl names its trait: {:?}",
            hellos.iter().map(|s| &s.detail).collect::<Vec<_>>()
        );
        // The trait's own method SIGNATURE is a member too, qualified by the
        // trait that owns it. Before this feature it was `function_signature_item`,
        // which no kinds-table arm matched — invisible to the index entirely.
        assert!(
            names(&e).contains(&("Greeter::hello", "method")),
            "{:?}",
            names(&e)
        );
        assert_eq!(
            find(&e, "Greeter::hello").detail,
            None,
            "a trait owns its own name"
        );
    }

    #[test]
    fn doc_summaries_are_first_line_only_and_absent_when_undocumented() {
        let e = extract("rust", RUST_FIXTURE).unwrap();
        assert_eq!(
            find(&e, "Config").doc.as_deref(),
            Some("Loads configuration.")
        );
        assert_eq!(
            find(&e, "Config::load").doc.as_deref(),
            Some("Reads the config from disk.")
        );
        assert_eq!(
            find(&e, "Config::DEFAULT").doc.as_deref(),
            Some("The default path.")
        );
        // An undocumented item, and one preceded by a PLAIN `//` comment,
        // both carry no summary — absent is absent.
        assert_eq!(find(&e, "Stack::push").doc, None);
        assert!(
            e.symbols
                .iter()
                .filter(|s| s.name == "Config::hello")
                .all(|s| s.doc.is_none()),
            "a // comment is not a doc comment"
        );
    }

    #[test]
    fn doc_summary_is_bounded() {
        let long = "x".repeat(DOC_SUMMARY_CAP + 50);
        let source = format!("/// {long}\npub fn wide() {{}}\n");
        let e = extract("rust", &source).unwrap();
        let doc = find(&e, "wide").doc.clone().unwrap();
        assert_eq!(doc.chars().count(), DOC_SUMMARY_CAP);
    }

    #[test]
    fn cfg_test_modules_yield_ranges_and_cfg_feature_does_not() {
        let e = extract("rust", RUST_FIXTURE).unwrap();
        assert_eq!(e.test_ranges.len(), 1, "{:?}", e.test_ranges);
        let range = e.test_ranges[0];
        let lines: Vec<&str> = RUST_FIXTURE.lines().collect();
        assert!(
            lines[range.start_line as usize - 1].contains("mod tests"),
            "range starts at the module: {:?}",
            lines[range.start_line as usize - 1]
        );
        assert!(range.end_line > range.start_line);
        // The cfg(feature) module is ordinary code and must not be in a range.
        let extra_line = lines
            .iter()
            .position(|l| l.contains("mod extra"))
            .map(|i| i as u32 + 1)
            .unwrap();
        assert!(
            !(range.start_line..=range.end_line).contains(&extra_line),
            "cfg(feature) is not test code"
        );
    }

    #[test]
    fn a_nested_cfg_test_module_is_still_a_range() {
        let e = extract(
            "rust",
            "pub mod outer {\n    #[cfg(test)]\n    mod inner {\n        #[test]\n        fn t() {}\n    }\n}\n",
        )
        .unwrap();
        assert_eq!(e.test_ranges.len(), 1, "{:?}", e.test_ranges);
        assert_eq!(e.test_ranges[0].start_line, 3);
    }

    #[test]
    fn other_languages_are_untouched_by_the_rust_rules() {
        // A TypeScript class method keeps its bare name and `method` kind:
        // qualification is a Rust-only rule (specs/009 FR-011).
        let e = extract(
            "typescript",
            "export class Circle {\n  area(): number { return 1; }\n}\n",
        )
        .unwrap();
        assert!(names(&e).contains(&("area", "method")), "{:?}", names(&e));
        assert!(e.test_ranges.is_empty());
        assert!(e.symbols.iter().all(|s| s.detail.is_none()));
    }

    #[test]
    fn target_is_already_junk() {
        // specs/009 R5: the chunk lists this as acceptance and it ALREADY
        // holds — this test verifies shipped behavior rather than claiming it.
        assert!(is_junk("target/debug/build/x.rs"));
        assert!(is_junk("crates/core/target/release/y.rs"));
        assert!(!is_junk("src/target_helper.rs"));
    }

    #[test]
    fn rust_symbols_and_imports() {
        let e = extract(
            "rust",
            "use std::fmt::Debug;\n\npub struct Point { x: i32 }\n\npub fn origin() -> Point { Point { x: 0 } }\n\npub trait Draw {}\n",
        )
        .unwrap();
        assert_eq!(
            names(&e),
            vec![
                ("Point", "struct"),
                ("origin", "function"),
                ("Draw", "trait")
            ]
        );
        assert_eq!(e.imports, vec!["std::fmt::Debug"]);
        let point = &e.symbols[0];
        assert_eq!((point.start_line, point.start_col), (3, 1));
    }

    #[test]
    fn typescript_symbols_and_imports() {
        let e = extract(
            "typescript",
            "import { x } from \"./x\";\nexport interface Shape { area(): number }\nexport type Id = string;\nexport const compute = (n: number) => n * 2;\nexport class Circle {\n  radius = 1;\n  area(): number { return 3 * this.radius; }\n}\nexport function main(): void {}\n",
        )
        .unwrap();
        let got = names(&e);
        assert!(got.contains(&("Shape", "interface")), "{got:?}");
        assert!(got.contains(&("Id", "type")), "{got:?}");
        assert!(got.contains(&("compute", "function")), "{got:?}");
        assert!(got.contains(&("Circle", "class")), "{got:?}");
        assert!(got.contains(&("area", "method")), "{got:?}");
        assert!(got.contains(&("main", "function")), "{got:?}");
        assert_eq!(e.imports, vec!["./x"]);
    }

    #[test]
    fn python_go_java_symbols_and_imports() {
        let py = extract(
            "python",
            "import os\nfrom collections import OrderedDict\n\nclass Cache:\n    def get(self, k):\n        return None\n\ndef main():\n    pass\n",
        )
        .unwrap();
        assert!(names(&py).contains(&("Cache", "class")));
        assert!(names(&py).contains(&("get", "function")));
        assert!(names(&py).contains(&("main", "function")));
        assert_eq!(py.imports, vec!["os", "collections"]);

        let go = extract(
            "go",
            "package main\n\nimport \"fmt\"\n\ntype Point struct{ X int }\n\nfunc (p Point) Norm() int { return p.X }\n\nfunc main() { fmt.Println(1) }\n",
        )
        .unwrap();
        assert!(names(&go).contains(&("Point", "type")));
        assert!(names(&go).contains(&("Norm", "method")));
        assert!(names(&go).contains(&("main", "function")));
        assert_eq!(go.imports, vec!["fmt"]);

        let java = extract(
            "java",
            "import java.util.List;\n\npublic class App {\n  public App() {}\n  int run() { return 0; }\n}\n",
        )
        .unwrap();
        assert!(names(&java).contains(&("App", "class")));
        assert!(names(&java).contains(&("run", "method")));
        assert_eq!(java.imports, vec!["java.util.List"]);
    }

    #[test]
    fn c_cpp_declarator_chains_and_includes() {
        let c = extract(
            "c",
            "#include <stdio.h>\n\nstruct point { int x; };\n\nint *lookup(int key) { return 0; }\n",
        )
        .unwrap();
        assert!(names(&c).contains(&("point", "struct")));
        assert!(names(&c).contains(&("lookup", "function")));
        assert_eq!(c.imports, vec!["stdio.h"]);

        let cpp = extract(
            "cpp",
            "#include \"lib.h\"\nnamespace geo {\nclass Circle { public: double area(); };\ndouble Circle::area() { return 3.0; }\n}\n",
        )
        .unwrap();
        assert!(names(&cpp).contains(&("geo", "namespace")));
        assert!(names(&cpp).contains(&("Circle", "class")));
        assert!(names(&cpp).contains(&("Circle::area", "function")));
        assert_eq!(cpp.imports, vec!["lib.h"]);
    }

    #[test]
    fn c_specifier_reference_is_not_a_declaration() {
        let e = extract("c", "struct point make_origin(void);\n").unwrap();
        assert!(
            !names(&e).iter().any(|(n, _)| *n == "point"),
            "a bodiless specifier is a reference, not a declaration: {e:?}"
        );
    }

    #[test]
    fn remaining_languages_extract() {
        let sh = extract("bash", "greet() {\n  echo hi\n}\n").unwrap();
        assert!(names(&sh).contains(&("greet", "function")));

        let rb = extract(
            "ruby",
            "require \"json\"\n\nmodule Api\n  class Client\n    def fetch\n    end\n  end\nend\n",
        )
        .unwrap();
        assert!(names(&rb).contains(&("Api", "module")));
        assert!(names(&rb).contains(&("Client", "class")));
        assert!(names(&rb).contains(&("fetch", "method")));
        assert_eq!(rb.imports, vec!["json"]);

        let cs = extract(
            "csharp",
            "using System.Text;\n\nnamespace App;\n\npublic class Runner {\n  public int Run() { return 0; }\n}\n",
        )
        .unwrap();
        assert!(names(&cs).contains(&("App", "namespace")));
        assert!(names(&cs).contains(&("Runner", "class")));
        assert!(names(&cs).contains(&("Run", "method")));
        assert_eq!(cs.imports, vec!["System.Text"]);

        // JSON declares nothing — and must not error.
        let json = extract("json", "{\"a\": 1}\n").unwrap();
        assert!(json.symbols.is_empty());
        assert!(json.imports.is_empty());

        let tsx = extract("tsx", "export function App() { return <div/>; }\n").unwrap();
        assert!(names(&tsx).contains(&("App", "function")));

        let js = extract(
            "javascript",
            "import util from \"node:util\";\nexport const handler = async (event) => event;\nfunction helper() {}\n",
        )
        .unwrap();
        assert!(names(&js).contains(&("handler", "function")));
        assert!(names(&js).contains(&("helper", "function")));
        assert_eq!(js.imports, vec!["node:util"]);
    }

    #[test]
    fn every_bundled_language_extracts_without_error() {
        for (id, _) in crate::langs::LANGUAGES {
            extract(id, "x\n").unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }

    #[test]
    fn broken_source_still_yields_intact_symbols() {
        let e = extract("typescript", "function ok() {}\nfunction broken( {\n").unwrap();
        assert!(names(&e).contains(&("ok", "function")));
    }

    #[test]
    fn junk_detection_flags_vendor_generated_and_lockfiles() {
        assert!(is_junk("node_modules/pkg/index.js"));
        assert!(is_junk("sub/vendor/lib.go"));
        assert!(is_junk("Cargo.lock"));
        assert!(is_junk("assets/app.min.js"));
        assert!(is_junk("build/out.map"));
        assert!(!is_junk("src/main.rs"));
        assert!(!is_junk("vendor.ts"));
        assert!(!is_junk("docs/vendor-notes.md"));
    }

    #[test]
    fn scan_respects_gitignore_and_flags_junk() {
        let root = std::env::temp_dir().join(format!("wb-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("ignored")).unwrap();
        std::fs::create_dir_all(root.join("vendor")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join(".workbench")).unwrap();
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(root.join("ignored/secret.txt"), "x\n").unwrap();
        std::fs::write(root.join("vendor/lib.js"), "var x\n").unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref\n").unwrap();
        std::fs::write(root.join(".workbench/index.db"), "db\n").unwrap();

        let entries = scan_workspace(root.to_str().unwrap()).unwrap();
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"src/main.rs"), "{paths:?}");
        assert!(paths.contains(&".gitignore"), "{paths:?}");
        assert!(paths.contains(&"vendor/lib.js"), "{paths:?}");
        assert!(
            !paths.iter().any(|p| p.starts_with("ignored/")),
            "{paths:?}"
        );
        assert!(!paths.iter().any(|p| p.starts_with(".git/")), "{paths:?}");
        assert!(
            !paths.iter().any(|p| p.starts_with(".workbench/")),
            "{paths:?}"
        );
        let vendor = entries.iter().find(|e| e.path == "vendor/lib.js").unwrap();
        assert!(vendor.junk);
        let main = entries.iter().find(|e| e.path == "src/main.rs").unwrap();
        assert!(!main.junk);
        assert!(main.size > 0);
        assert!(main.mtime_ms > 0);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
