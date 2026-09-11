//! SC-002 (specs/002-wasm-lang-pack, chunk:m5-lang-pack acceptance 2): the
//! twelve bundled grammars stay native and the lang-pack registry is
//! consulted only after a bundled-table miss — pinned structurally AND by
//! measurement, because Constitution VI says a perf claim ships with its
//! number.

#![cfg(feature = "lang-pack")]

use std::time::{Duration, Instant};

use workbench_core::{engine, indexer, lang_registry};

const GLEAM_WASM: &[u8] = include_bytes!("../../../plugins/lang-pack-gleam/grammar/gleam.wasm");
const GLEAM_TAGS: &str = include_str!("../../../plugins/lang-pack-gleam/queries/tags.scm");

/// A representative bundled-language source: big enough that parse time is
/// dominated by parsing, small enough to iterate.
fn ts_corpus() -> String {
    let mut out = String::new();
    for i in 0..120 {
        out.push_str(&format!(
            "export interface Shape{i} {{ area(): number; name: string }}\n\
             export function make{i}(n: number): Shape{i} {{\n\
               return {{ area: () => n * {i}, name: \"s{i}\" }};\n\
             }}\n\
             export const c{i} = make{i}({i});\n",
        ));
    }
    out
}

fn register_gleam() {
    lang_registry::register(
        "gleam",
        &["gleam".to_string()],
        GLEAM_WASM,
        GLEAM_TAGS,
        false,
    )
    .expect("gleam pack registers");
}

/// Structural half: the bundled parse and extract paths complete while the
/// registry's write lock is HELD. The lock is not reentrant, so any registry
/// consultation on these paths would deadlock — completion is the proof that
/// the bundled table is checked first and the registry only on a miss.
#[test]
fn bundled_paths_never_consult_the_registry() {
    let source = ts_corpus();
    lang_registry::with_registry_locked(|| {
        let summary = engine::parse("typescript", &source).expect("bundled parse");
        assert!(!summary.has_error);
        let extraction = indexer::extract("typescript", &source).expect("bundled extract");
        assert!(!extraction.symbols.is_empty());
    });
}

/// Measured half: bundled parse cost with a registered pack stays within
/// noise of the empty-registry cost. Threshold 1.5x on the min-of-rounds —
/// generous against scheduler noise (both sides are the same build, same
/// process), while an accidental registry hop on the hot path (lock + map
/// lookup per parse) would not hide under it. Registry-consulted parses of
/// the same corpus cost 2–5x, the documented long-tail price.
///
/// One test body with the wasm-lane probe below it: the registry is process
/// global, and parallel test threads clearing it under each other would race.
#[test]
fn bundled_parse_cost_is_unchanged_by_a_registered_pack() {
    let source = ts_corpus();
    let rounds = 3usize;
    let iters = 12usize;

    let measure = |label: &str| -> Duration {
        let mut best = Duration::MAX;
        for _ in 0..rounds {
            let start = Instant::now();
            for _ in 0..iters {
                let s = engine::parse("typescript", &source).expect(label);
                assert!(!s.has_error);
            }
            best = best.min(start.elapsed());
        }
        best
    };

    lang_registry::clear();
    let empty = measure("empty-registry parse");

    register_gleam();
    let registered = measure("registered-pack parse");
    lang_registry::clear();

    let ratio = registered.as_secs_f64() / empty.as_secs_f64().max(f64::EPSILON);
    assert!(
        ratio < 1.5,
        "bundled parse slowed with a registered pack: empty={empty:?} registered={registered:?} ratio={ratio:.2}"
    );

    // The other side of the coin, kept honest while the pack is registered:
    // the pack language itself parses on the WasmStore lane — the accepted
    // long-tail cost exists and is real, it just never lands on the bundled
    // path.
    register_gleam();
    let gleam = "pub fn main() {\n  1\n}\n".repeat(50);
    let summary = lang_registry::parse("gleam", &gleam)
        .expect("registered")
        .expect("parses");
    assert!(!summary.has_error);
    lang_registry::clear();
}
