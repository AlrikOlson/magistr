//! The bundled visibility ranking signal (chunk:m5-ranking-shadow): the first
//! first-party ranking signal on the public `magistr:ranking@0.1.0`
//! interface. One documented feature, `exported` (host-namespaced to
//! `ranking-visibility/exported`): 1.0 when a declaration hit's excerpt
//! carries an export/public visibility marker, 0.0 for other declarations,
//! nothing for non-declaration hits — exported-ness is a relevance fact the
//! first-party table (kind/file-class/path) does not already encode.
//!
//! Detection is hand-rolled token matching (no regex crate in no_std; the
//! think:119 recipe keeps this module importing NOTHING so its declared
//! capability set stays honestly empty).

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
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
        world: "ranking-visibility",
        generate_all,
        std_feature,
    });
}

use bindings::exports::magistr::ranking::ranking::{
    Feature, Guest as RankingGuest, HitContext, KindClass,
};

struct Component;

impl bindings::Guest for Component {
    /// The standard plugin-tool convention (TCK conformance): identity JSON.
    fn invoke(_payload: String) -> String {
        r#"{"signal":"ranking-visibility","surface":"magistr:ranking@0.1.0","features":["exported"]}"#
            .to_string()
    }
}

/// Visibility markers that make a declaration "exported" when they appear as
/// one of the excerpt's leading word tokens. Leading, because `export`/`pub`/
/// `public` are declaration modifiers — a mention later in the line (a string,
/// a comment, an argument) is not a visibility claim about this declaration.
const MARKERS: [&str; 3] = ["export", "pub", "public"];

/// How many leading word tokens to inspect. Modifiers stack (`export default
/// async function`, `public static final class`), but never deeper than this.
const LEADING_TOKENS: usize = 4;

fn is_exported(excerpt: &str) -> bool {
    excerpt
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .take(LEADING_TOKENS)
        .any(|token| MARKERS.contains(&token))
}

impl RankingGuest for Component {
    /// Features only, by construction: the interface has no ordering verb.
    /// Non-declaration hits contribute nothing; declarations report whether
    /// they are exported (1.0) or not (0.0), so the combiner can weight both.
    fn contribute(hit: HitContext) -> Vec<Feature> {
        if !matches!(hit.kind_class, KindClass::Declaration) {
            return Vec::new();
        }
        let value = if is_exported(&hit.excerpt) { 1.0 } else { 0.0 };
        vec![Feature {
            name: "exported".to_string(),
            value,
        }]
    }
}

bindings::export!(Component with_types_in bindings);
