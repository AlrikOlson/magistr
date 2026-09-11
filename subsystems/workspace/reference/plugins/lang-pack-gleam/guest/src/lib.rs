//! The bundled gleam lang pack (chunk:m5-lang-pack): the first first-party
//! language pack on the public `workbench:lang-pack@0.1.0` interface — a
//! CARRIER component (specs/002-wasm-lang-pack/research.md R1). It hands the
//! host the tree-sitter grammar compiled to WASM, the tag queries feeding
//! the Tier-1 symbol indexer, and a file-class heuristic; the grammar
//! executes in the engine child's registry lanes, never here.
//!
//! no_std by the think:119 recipe so the module imports NOTHING — this
//! guest declares zero capabilities and the sandbox's default-deny has
//! nothing to even consider.

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// wasi-libc normally provides `memcmp`; without std nothing does.
#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let x = unsafe { *a.add(i) };
        let y = unsafe { *b.add(i) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
        i += 1;
    }
    0
}

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "lang-pack-gleam",
        generate_all,
        std_feature,
    });
}

use bindings::exports::workbench::lang_pack::lang_pack::{
    FileClass, Guest as LangPackGuest, LanguageInfo,
};

/// The committed grammar artifact (grammar/README.md documents the build).
const GRAMMAR_WASM: &[u8] = include_bytes!("../../grammar/gleam.wasm");
/// The tag queries feeding the Tier-1 symbol indexer for gleam.
const TAG_QUERIES: &str = include_str!("../../queries/tags.scm");

struct Component;

impl bindings::Guest for Component {
    /// The standard plugin-tool convention (TCK conformance): identity JSON.
    fn invoke(_payload: String) -> String {
        r#"{"lang-pack":"gleam","surface":"workbench:lang-pack@0.1.0"}"#.to_string()
    }
}

fn ends_with(path: &str, suffix: &str) -> bool {
    path.len() >= suffix.len() && path[path.len() - suffix.len()..] == *suffix
}

fn starts_with(path: &str, prefix: &str) -> bool {
    path.len() >= prefix.len() && path[..prefix.len()] == *prefix
}

fn contains(path: &str, needle: &str) -> bool {
    if needle.len() > path.len() {
        return false;
    }
    let bytes = path.as_bytes();
    let n = needle.as_bytes();
    (0..=bytes.len() - n.len()).any(|i| &bytes[i..i + n.len()] == n)
}

impl LangPackGuest for Component {
    fn info() -> LanguageInfo {
        let extensions = alloc::vec!["gleam".to_string()];
        LanguageInfo {
            name: "gleam".to_string(),
            extensions,
        }
    }

    fn grammar_wasm() -> Vec<u8> {
        GRAMMAR_WASM.to_vec()
    }

    fn tag_queries() -> String {
        TAG_QUERIES.to_string()
    }

    /// Gleam's project layout is rigid enough for an honest heuristic:
    /// `build/` holds compiled output and downloaded deps (vendor), `test/`
    /// and `*_test.gleam` are tests, other `.gleam` files are source. `none`
    /// defers everything else to the host (the pack only answers where it
    /// knows better — wit/lang-pack.wit).
    fn classify_file(path: String) -> Option<FileClass> {
        let mut lower = String::with_capacity(path.len());
        for c in path.chars() {
            for l in c.to_lowercase() {
                lower.push(l);
            }
        }
        let p = lower.as_str();
        if !ends_with(p, ".gleam") {
            return None;
        }
        if starts_with(p, "build/") || contains(p, "/build/") {
            return Some(FileClass::Vendor);
        }
        if ends_with(p, "_test.gleam") || starts_with(p, "test/") || contains(p, "/test/") {
            return Some(FileClass::Test);
        }
        Some(FileClass::Source)
    }
}

bindings::export!(Component with_types_in bindings);
