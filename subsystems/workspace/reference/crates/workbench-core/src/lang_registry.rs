//! Dynamic language registry for WASM lang packs (SDD §8.3, feature 002,
//! chunk:m5-lang-pack).
//!
//! Pack languages live HERE, beside — never inside — the static bundled
//! table in [`crate::langs`]: registration refuses any claim that would
//! shadow a bundled id or extension, so the hot path's first branch
//! (`WbLang::get`) is byte-identical to the pre-pack build and the registry
//! is consulted only after a bundled miss (research R7, SC-002).
//!
//! Two execution lanes (research R5): the WASM lane drives the pack's
//! grammar bytes through tree-sitter's own `WasmStore` — note the engine
//! type below is `tree_sitter::wasmtime` (the c-api's wasmtime 36), NEVER
//! the sandbox host's wasmtime 48; the two majors coexist as distinct
//! crates (research R2). The native lane is the blessing-gated promotion:
//! a blessed pack whose language this build carries as a native grammar
//! crate runs on the same `LanguageFn` path as the bundled twelve.
//!
//! Extraction for registry languages is tag-query-driven (research R4): the
//! pack's `.scm` compiles once at registration; captures named `name` carry
//! the symbol name, the enclosing `@definition.<kind>` capture carries the
//! span and kind, and `@import` captures collect import targets. A query
//! that fails to compile degrades that language to no-symbols (parse-verify
//! stays live) — never a registration failure (spec edge cases).

use std::ops::ControlFlow;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use streaming_iterator::StreamingIterator;
use tree_sitter::wasmtime::Engine as GrammarEngine;
use tree_sitter::{Language, ParseOptions, Parser, Query, QueryCursor, Tree, WasmStore};

use crate::engine::{CoreError, ParseSummary};
use crate::indexer::{Extraction, Symbol};

/// Wall-clock budget for one pack-language parse. The WASM lane runs
/// unaudited grammar code; the progress callback bounds it the way the
/// sandbox's fuel bounds plugin code (spec edge "grammar executes
/// pathologically").
const PARSE_BUDGET: Duration = Duration::from_secs(2);

/// Which substrate executes a registered language (research R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Blessed + native grammar present: the same `LanguageFn` path as the
    /// bundled grammars.
    Native,
    /// The default long-tail lane: grammar bytes in tree-sitter's WasmStore,
    /// the documented, accepted 2–5x cost.
    Wasm,
}

impl Lane {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Wasm => "wasm",
        }
    }
}

/// What registration decided, surfaced verbatim to the host (map inventory).
#[derive(Debug, Clone)]
pub struct RegistrationOutcome {
    pub id: String,
    pub extensions: Vec<String>,
    pub lane: Lane,
    pub queries_ok: bool,
    /// Every shadowed claim, refusal detail, or degradation — observable,
    /// never silent (FR-012).
    pub notes: Vec<String>,
}

struct ParserSlot {
    parser: Parser,
}

struct Entry {
    id: String,
    extensions: Vec<String>,
    lane: Lane,
    /// The parser (wasm-store-armed on the wasm lane) is serialized: pack
    /// languages are the long tail, and one slot per language keeps the
    /// WasmStore lifetime simple.
    slot: Mutex<ParserSlot>,
    query: Option<Query>,
}

fn registry() -> &'static RwLock<Vec<Entry>> {
    static REGISTRY: OnceLock<RwLock<Vec<Entry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// The grammar engine (tree-sitter's wasmtime 36), initialised once. Traps
/// are native faults in every wasmtime; this engine and the sandbox host's
/// wasmtime 48 must both use chaining POSIX signal handlers rather than the
/// macOS default of one process-wide Mach exception port, or a trap raised by
/// whichever engine did NOT install the port last kills the process with
/// SIGILL (see `sandbox::engine`). `pub(crate)` so the sandbox regression test
/// can warm this engine before trapping a guest in the other one.
pub(crate) fn grammar_engine() -> &'static GrammarEngine {
    static ENGINE: OnceLock<GrammarEngine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = tree_sitter::wasmtime::Config::new();
        config.macos_use_mach_ports(false);
        // A default-shaped config cannot fail to build an engine; falling
        // back to `default()` here would silently reinstate the Mach port.
        GrammarEngine::new(&config).expect("grammar engine with POSIX trap handling")
    })
}

/// The native promotion table: pack languages this build also carries as
/// native grammar crates (research R5). Adding a language here is a build
/// decision, which is exactly what "blessed to native execution" promotes.
fn native_grammar(id: &str) -> Option<tree_sitter_language::LanguageFn> {
    match id {
        "gleam" => Some(tree_sitter_gleam::LANGUAGE),
        _ => None,
    }
}

/// The promotion decision (research R5), pure so its whole matrix is
/// testable without a second grammar: blessed + native grammar present →
/// native execution; blessed without one → wasm with an observable note
/// (spec edge case); any other tier → wasm, unconditionally.
fn select_lane(blessed: bool, has_native: bool, id: &str) -> (Lane, Option<String>) {
    match (blessed, has_native) {
        (true, true) => (Lane::Native, None),
        (true, false) => (
            Lane::Wasm,
            Some(format!(
                "blessed, but this build carries no native grammar for `{id}`; staying on the wasm lane"
            )),
        ),
        (false, _) => (Lane::Wasm, None),
    }
}

fn pack_rejected(id: &str, detail: String) -> CoreError {
    CoreError::LanguageRejected {
        id: id.to_string(),
        detail,
    }
}

/// Register (or re-register) one pack language. `blessed` is the host's
/// trust-tier verdict; it selects the lane, never the admission — any tier
/// may run on the WASM lane (SDD §8.3: sandboxing is by construction, not
/// review).
pub fn register(
    id: &str,
    claimed_extensions: &[String],
    grammar_wasm: &[u8],
    tag_queries: &str,
    blessed: bool,
) -> Result<RegistrationOutcome, CoreError> {
    let mut notes = Vec::new();

    if crate::langs::WbLang::get(id).is_some() {
        return Err(pack_rejected(
            id,
            format!("pack language id `{id}` shadows a bundled grammar; refused"),
        ));
    }

    let bundled_exts = bundled_extensions();
    let mut extensions: Vec<String> = Vec::new();
    for ext in claimed_extensions {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        if ext.is_empty() {
            continue;
        }
        if bundled_exts.contains(&ext.as_str()) {
            notes.push(format!(
                "extension `{ext}` is claimed by a bundled grammar; the pack's claim is ignored"
            ));
            continue;
        }
        if !extensions.contains(&ext) {
            extensions.push(ext);
        }
    }
    if extensions.is_empty() {
        return Err(pack_rejected(
            id,
            "pack claims no usable extensions (empty or all bundled-shadowed)".to_string(),
        ));
    }

    // Inter-pack conflicts: first registration (stable install order) wins.
    {
        let entries = registry().read().expect("registry poisoned");
        for other in entries.iter().filter(|e| e.id != id) {
            for ext in &extensions {
                if other.extensions.contains(ext) {
                    notes.push(format!(
                        "extension `{ext}` already claimed by pack `{}`; this pack loses it",
                        other.id
                    ));
                }
            }
        }
    }
    let taken: Vec<String> = {
        let entries = registry().read().expect("registry poisoned");
        entries
            .iter()
            .filter(|e| e.id != id)
            .flat_map(|e| e.extensions.iter().cloned())
            .collect()
    };
    extensions.retain(|e| !taken.contains(e));
    if extensions.is_empty() {
        return Err(pack_rejected(
            id,
            "every claimed extension is already owned by another pack".to_string(),
        ));
    }

    let (lane, note) = select_lane(blessed, native_grammar(id).is_some(), id);
    if let Some(note) = note {
        notes.push(note);
    }
    let (language, parser) = if lane == Lane::Native {
        let lang_fn = native_grammar(id).expect("Lane::Native implies a native grammar");
        let language: Language = lang_fn.into();
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| pack_rejected(id, format!("native grammar rejected: {e}")))?;
        (language, parser)
    } else {
        let mut store = WasmStore::new(grammar_engine())
            .map_err(|e| pack_rejected(id, format!("wasm store: {}", e.message)))?;
        let language = store
            .load_language(id, grammar_wasm)
            .map_err(|e| pack_rejected(id, format!("grammar wasm rejected: {}", e.message)))?;
        let mut parser = Parser::new();
        parser
            .set_wasm_store(store)
            .map_err(|e| pack_rejected(id, format!("wasm store rejected: {e}")))?;
        parser
            .set_language(&language)
            .map_err(|e| pack_rejected(id, format!("wasm grammar rejected: {e}")))?;
        (language, parser)
    };

    let query = match Query::new(&language, tag_queries) {
        Ok(q) => Some(q),
        Err(e) => {
            notes.push(format!(
                "tag queries failed to compile ({e}); `{id}` degrades to no-symbols (parse-verify stays live)"
            ));
            None
        }
    };

    let outcome = RegistrationOutcome {
        id: id.to_string(),
        extensions: extensions.clone(),
        lane,
        queries_ok: query.is_some(),
        notes,
    };

    let entry = Entry {
        id: id.to_string(),
        extensions,
        lane,
        slot: Mutex::new(ParserSlot { parser }),
        query,
    };
    let mut entries = registry().write().expect("registry poisoned");
    entries.retain(|e| e.id != entry.id);
    entries.push(entry);
    Ok(outcome)
}

/// SC-002's structural probe (specs/002-wasm-lang-pack): run `f` while
/// holding the registry's write lock. The lock is not reentrant, so if any
/// code path inside `f` consulted the registry it would deadlock — a
/// completing `f` therefore proves that path never touches pack state.
/// Test support; useless (but harmless) elsewhere.
pub fn with_registry_locked<T>(f: impl FnOnce() -> T) -> T {
    let _guard = registry().write().expect("registry poisoned");
    f()
}

/// Drop every registered language (tests, and host-driven re-registration).
pub fn clear() {
    registry().write().expect("registry poisoned").clear();
}

/// Registered ids, registration order.
pub fn registered_ids() -> Vec<String> {
    registry()
        .read()
        .expect("registry poisoned")
        .iter()
        .map(|e| e.id.clone())
        .collect()
}

/// Provenance snapshot for the host's inventory: (id, extensions, lane,
/// queries_ok).
pub fn snapshot() -> Vec<(String, Vec<String>, &'static str, bool)> {
    registry()
        .read()
        .expect("registry poisoned")
        .iter()
        .map(|e| {
            (
                e.id.clone(),
                e.extensions.clone(),
                e.lane.as_str(),
                e.query.is_some(),
            )
        })
        .collect()
}

/// The registered language claiming `ext` (lowercase, no dot), if any.
/// Callers resolve bundled extensions FIRST — registration already refuses
/// bundled shadowing, so this cannot contradict the bundled table.
pub fn language_for_extension(ext: &str) -> Option<String> {
    let ext = ext.to_ascii_lowercase();
    registry()
        .read()
        .expect("registry poisoned")
        .iter()
        .find(|e| e.extensions.contains(&ext))
        .map(|e| e.id.clone())
}

fn with_entry<T>(id: &str, f: impl FnOnce(&Entry) -> T) -> Option<T> {
    let entries = registry().read().expect("registry poisoned");
    entries.iter().find(|e| e.id == id).map(f)
}

fn parse_tree(slot: &mut ParserSlot, source: &str) -> Option<Tree> {
    let deadline = Instant::now() + PARSE_BUDGET;
    let mut progress = |_: &tree_sitter::ParseState| {
        if Instant::now() >= deadline {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let options = ParseOptions::new().progress_callback(&mut progress);
    let bytes = source.as_bytes();
    let mut read = |offset: usize, _pos: tree_sitter::Point| &bytes[offset.min(bytes.len())..];
    slot.parser
        .parse_with_options(&mut read, None, Some(options))
}

/// Parse `source` with the registered grammar `id`. `None` when `id` is not
/// registered (the caller then reports unknown-language exactly as before).
pub fn parse(id: &str, source: &str) -> Option<Result<ParseSummary, CoreError>> {
    with_entry(id, |entry| {
        let mut slot = entry.slot.lock().expect("parser slot poisoned");
        let Some(tree) = parse_tree(&mut slot, source) else {
            return Err(CoreError::ParseFailed { id: id.to_string() });
        };
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
            language: id.to_string(),
            root_kind: root.kind().to_string(),
            node_count: u32::try_from(root.descendant_count()).unwrap_or(u32::MAX),
            error_count,
            has_error: root.has_error(),
        })
    })
}

/// Map a `definition.<kind>` capture name to the indexer's static kind
/// vocabulary. Unknown kinds fall back to `symbol` rather than dropping the
/// capture — a pack author's new kind should degrade visibly, not vanish.
fn capture_kind(name: &str) -> Option<&'static str> {
    let kind = name.strip_prefix("definition.")?;
    Some(match kind {
        "function" => "function",
        "method" => "method",
        "class" => "class",
        "module" => "module",
        "interface" => "interface",
        "type" => "type",
        "struct" => "struct",
        "enum" => "enum",
        "trait" => "trait",
        "macro" => "macro",
        "namespace" => "namespace",
        "constant" | "const" => "const",
        // Callable value constructors (e.g. gleam data constructors).
        "constructor" => "function",
        _ => "symbol",
    })
}

/// Extract symbols and imports from `source` via the pack's tag queries.
/// `None` when `id` is not registered. A registered language with no
/// compiled query answers an empty extraction (the documented degradation),
/// never an error.
pub fn extract(id: &str, source: &str) -> Option<Result<Extraction, CoreError>> {
    with_entry(id, |entry| {
        let mut slot = entry.slot.lock().expect("parser slot poisoned");
        let Some(tree) = parse_tree(&mut slot, source) else {
            return Err(CoreError::ParseFailed { id: id.to_string() });
        };
        let mut extraction = Extraction::default();
        let Some(query) = entry.query.as_ref() else {
            return Ok(extraction);
        };

        let names = query.capture_names();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let mut name_text: Option<String> = None;
            let mut definition: Option<(&'static str, tree_sitter::Node)> = None;
            for capture in m.captures {
                let capture_name = names[capture.index as usize];
                if capture_name == "name" {
                    name_text = capture
                        .node
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty());
                } else if capture_name == "import" {
                    if let Ok(text) = capture.node.utf8_text(source.as_bytes()) {
                        let target = crate::indexer::strip_delimiters(text);
                        if !target.is_empty() && !extraction.imports.contains(&target) {
                            extraction.imports.push(target);
                        }
                    }
                } else if let Some(kind) = capture_kind(capture_name) {
                    definition = Some((kind, capture.node));
                }
            }
            if let (Some(name), Some((kind, node))) = (name_text, definition) {
                let start = node.start_position();
                let end = node.end_position();
                extraction.symbols.push(Symbol {
                    name,
                    kind,
                    start_line: u32::try_from(start.row + 1).unwrap_or(u32::MAX),
                    start_col: u32::try_from(start.column + 1).unwrap_or(u32::MAX),
                    end_line: u32::try_from(end.row + 1).unwrap_or(u32::MAX),
                    // A pack's tag queries carry no qualification or doc
                    // capture (specs/009 is bundled-Rust only); both stay
                    // absent rather than guessed.
                    detail: None,
                    doc: None,
                });
            }
        }
        Ok(extraction)
    })
}

/// Every extension owned by a bundled grammar: the shadow list registration
/// enforces (FR-005). Mirrors the host-side extension map; a pack claiming
/// any of these is noted and ignored.
fn bundled_extensions() -> &'static [&'static str] {
    &[
        "sh", "bash", "c", "h", "cpp", "cc", "cxx", "hpp", "hh", "cs", "go", "java", "js", "mjs",
        "cjs", "jsx", "json", "py", "pyi", "rb", "rs", "ts", "mts", "cts", "tsx",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed first-party grammar artifact: the wasm-lane tests are
    /// exactly as hermetic as the committed guest components.
    const GLEAM_WASM: &[u8] = include_bytes!("../../../plugins/lang-pack-gleam/grammar/gleam.wasm");
    const GLEAM_TAGS: &str = include_str!("../../../plugins/lang-pack-gleam/queries/tags.scm");

    const FIXTURE: &str = "import gleam/io\n\npub fn main() {\n  io.println(greet(\"world\"))\n}\n\nfn greet(name: String) -> String {\n  \"hello\"\n}\n\npub type Season {\n  Spring\n  Summer\n}\n\npub const answer = 42\n";

    /// The registry is process-global and cargo runs tests on parallel
    /// threads: every test that registers or clears holds this guard, or a
    /// sibling's `clear()` lands between its register and its extract.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: Mutex<()> = Mutex::new(());
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn register_gleam(blessed: bool) -> RegistrationOutcome {
        clear();
        register(
            "gleam",
            &["gleam".to_string()],
            GLEAM_WASM,
            GLEAM_TAGS,
            blessed,
        )
        .unwrap()
    }

    #[test]
    fn wasm_lane_parses_and_extracts() {
        let _guard = serial();
        let outcome = register_gleam(false);
        assert_eq!(outcome.lane, Lane::Wasm);
        assert!(outcome.queries_ok, "{:?}", outcome.notes);

        let summary = parse("gleam", FIXTURE).unwrap().unwrap();
        assert!(!summary.has_error, "{summary:?}");
        assert!(summary.node_count > 10);

        let extraction = extract("gleam", FIXTURE).unwrap().unwrap();
        let names: Vec<(&str, &str)> = extraction
            .symbols
            .iter()
            .map(|s| (s.name.as_str(), s.kind))
            .collect();
        assert!(names.contains(&("main", "function")), "{names:?}");
        assert!(names.contains(&("greet", "function")), "{names:?}");
        assert!(names.contains(&("Season", "type")), "{names:?}");
        assert!(
            extraction.imports.contains(&"gleam/io".to_string()),
            "{extraction:?}"
        );
        clear();
    }

    #[test]
    fn blessed_pack_promotes_to_native_and_extraction_is_identical() {
        let _guard = serial();
        let wasm_outcome = register_gleam(false);
        assert_eq!(wasm_outcome.lane, Lane::Wasm);
        let via_wasm = extract("gleam", FIXTURE).unwrap().unwrap();
        let wasm_summary = parse("gleam", FIXTURE).unwrap().unwrap();

        let native_outcome = register_gleam(true);
        assert_eq!(
            native_outcome.lane,
            Lane::Native,
            "the tier flip changes the lane"
        );
        let via_native = extract("gleam", FIXTURE).unwrap().unwrap();
        let native_summary = parse("gleam", FIXTURE).unwrap().unwrap();

        // SC-003 / FR-008: identical extraction across lanes.
        assert_eq!(via_wasm, via_native);
        assert_eq!(wasm_summary.has_error, native_summary.has_error);
        assert_eq!(wasm_summary.node_count, native_summary.node_count);
        clear();
    }

    #[test]
    fn syntax_error_is_reported_for_edit_verification() {
        let _guard = serial();
        register_gleam(false);
        let broken = parse("gleam", "pub fn broken( {\n").unwrap().unwrap();
        assert!(broken.has_error);
        assert!(broken.error_count > 0);
        clear();
    }

    #[test]
    fn bundled_shadowing_is_refused_and_noted() {
        let _guard = serial();
        clear();
        // A pack claiming a bundled id is refused outright.
        let err = register("rust", &["gleam".to_string()], GLEAM_WASM, "", false).unwrap_err();
        assert!(err.to_string().contains("shadows a bundled grammar"));

        // A pack claiming a bundled extension loses that claim with a note.
        let outcome = register(
            "gleam",
            &["rs".to_string(), "gleam".to_string()],
            GLEAM_WASM,
            GLEAM_TAGS,
            false,
        )
        .unwrap();
        assert_eq!(outcome.extensions, vec!["gleam".to_string()]);
        assert!(
            outcome.notes.iter().any(|n| n.contains("`rs`")),
            "{:?}",
            outcome.notes
        );

        // Only bundled-shadowed claims → refused.
        let err = register("gloom", &["rs".to_string()], GLEAM_WASM, "", false).unwrap_err();
        assert!(err.to_string().contains("no usable extensions"));
        clear();
    }

    #[test]
    fn malformed_grammar_and_bad_queries_degrade_observably() {
        let _guard = serial();
        clear();
        let err = register("gleam", &["gleam".to_string()], b"not wasm", "", false).unwrap_err();
        assert!(err.to_string().contains("grammar wasm rejected"), "{err}");

        // Bad queries: registration succeeds, symbols degrade, parse lives.
        let outcome = register(
            "gleam",
            &["gleam".to_string()],
            GLEAM_WASM,
            "(nonexistent_node) @name",
            false,
        )
        .unwrap();
        assert!(!outcome.queries_ok);
        assert!(outcome.notes.iter().any(|n| n.contains("tag queries")));
        let extraction = extract("gleam", FIXTURE).unwrap().unwrap();
        assert!(extraction.symbols.is_empty());
        assert!(!parse("gleam", FIXTURE).unwrap().unwrap().has_error);
        clear();
    }

    #[test]
    fn lane_selection_matrix_including_blessing_without_a_native_grammar() {
        // Blessed + native present: promoted, silently.
        let (lane, note) = select_lane(true, true, "gleam");
        assert_eq!(lane, Lane::Native);
        assert!(note.is_none());
        // Blessed without a native grammar: wasm lane with an observable note
        // (spec edge case). A live probe of this arm needs a second grammar —
        // WasmStore couples the load name to the grammar's exported symbol —
        // so the decision is pure and pinned here.
        let (lane, note) = select_lane(true, false, "gloom");
        assert_eq!(lane, Lane::Wasm);
        assert!(note.unwrap().contains("no native grammar"));
        // Any other tier: wasm, unconditionally.
        for has_native in [true, false] {
            let (lane, note) = select_lane(false, has_native, "gleam");
            assert_eq!(lane, Lane::Wasm);
            assert!(note.is_none());
        }
    }

    #[test]
    fn unregistered_language_answers_none() {
        let _guard = serial();
        clear();
        assert!(parse("cobol", "x").is_none());
        assert!(extract("cobol", "x").is_none());
        assert!(language_for_extension("cob").is_none());
    }
}
