use super::*;

use crate::fonts::EmbeddedFonts;
use crate::metrics::CellMetrics;

fn engine() -> TextEngine {
    TextEngine::embedded()
}

fn size() -> PxSize {
    PxSize::new(CellMetrics::DEFAULT_PX)
}

/// A monospaced run advances by exactly one cell per character, which is
/// what lets the painter emit a whole row as one shaped run and still
/// have its glyphs land on the grid the layout engine measured in cells.
#[test]
fn a_monospaced_run_advances_one_cell_per_character() {
    let mut engine = engine();
    let cell = engine.metrics(CellMetrics::DEFAULT_PX).expect("metrics");
    let text = "magistr";
    let run = engine.run(text, EmbeddedFonts::REGULAR, size(), Features::default());
    assert_eq!(run.glyphs.len(), text.chars().count());
    assert_eq!(run.missing_glyphs, 0);
    let expected = cell.width() * text.chars().count() as f32;
    assert!(
        (run.width - expected).abs() <= 1.0,
        "run width {} against {expected} cells",
        run.width
    );
}

/// The cache is what keeps a steady cockpit from re-shaping a screen of
/// unchanged rows every frame.
#[test]
fn an_unchanged_row_is_shaped_once_and_then_cached() {
    let mut engine = engine();
    let first = engine.run(
        "the same row",
        EmbeddedFonts::REGULAR,
        size(),
        Features::default(),
    );
    let second = engine.run(
        "the same row",
        EmbeddedFonts::REGULAR,
        size(),
        Features::default(),
    );
    assert!(Arc::ptr_eq(&first, &second), "the second call re-shaped");
}

/// Bold is a different face, so it is a different shaped run — the two
/// must not collide on one cache slot.
#[test]
fn bold_and_regular_are_separate_runs() {
    let mut engine = engine();
    let regular = engine.run("gate", EmbeddedFonts::REGULAR, size(), Features::default());
    let bold = engine.run("gate", EmbeddedFonts::BOLD, size(), Features::default());
    assert!(!Arc::ptr_eq(&regular, &bold));
}

/// A glyph with ink is admitted with a bitmap; a space is `None` and is
/// *not* a failure — the run's advance carries it, so the painter draws
/// nothing and the next glyph still lands in its own column.
#[test]
fn ink_is_admitted_and_a_space_draws_nothing() {
    let mut engine = engine();
    engine.begin_frame();
    let run = engine.run("A ", EmbeddedFonts::REGULAR, size(), Features::default());
    let mut drawn = 0;
    for glyph in &run.glyphs {
        if let Some(placed) = engine.glyph(glyph.font, glyph.glyph_id, size(), glyph.x) {
            assert!(placed.entry.width > 0 && placed.entry.height > 0);
            drawn += 1;
        }
    }
    assert_eq!(drawn, 1, "the space has no ink to admit");
}

/// The pen position handed back is a whole pixel — the fractional part
/// went into which subpixel phase was rasterized, which is the entire
/// reason `GlyphKey::quantize` returns two values.
#[test]
fn the_pen_position_is_a_whole_pixel_and_the_fraction_is_in_the_phase() {
    let mut engine = engine();
    engine.begin_frame();
    let id = glyph_id_of_a(&mut engine);
    let placed = engine
        .glyph(EmbeddedFonts::REGULAR, id, size(), 10.4)
        .expect("A has ink");
    assert_eq!(placed.pen, 10.0);
}

fn glyph_id_of_a(engine: &mut TextEngine) -> u16 {
    engine
        .run("A", EmbeddedFonts::REGULAR, size(), Features::default())
        .glyphs[0]
        .glyph_id
}

/// One frame admits the cockpit's whole alphabet: the bound is raised
/// deliberately (see `TextEngine::UPLOADS`), because a golden of a
/// converging atlas is a picture of the atlas rather than of the UI.
#[test]
fn one_frame_admits_a_whole_screens_worth_of_distinct_glyphs() {
    let mut engine = engine();
    engine.begin_frame();
    let alphabet: String = (b' '..=b'~').map(char::from).collect();
    let run = engine.run(
        &alphabet,
        EmbeddedFonts::REGULAR,
        size(),
        Features::default(),
    );
    for glyph in &run.glyphs {
        engine.glyph(glyph.font, glyph.glyph_id, size(), glyph.x);
    }
    assert!(
        engine.converged(),
        "the atlas deferred part of one screen's alphabet"
    );
    assert!(!engine.take_uploads().is_empty());
}

/// The two atlas pages are different sizes, and a `uv` is normalized
/// against the page it came from — so a CPU mirror sized from the wrong
/// one reads the right texture at the wrong coordinates.
#[test]
fn the_coverage_and_colour_pages_report_their_own_edges() {
    let engine = engine();
    assert_eq!(engine.atlas_size(), TextEngine::ATLAS);
    assert_eq!(engine.colour_size(), TextEngine::COLOUR);
    // The page has to hold a screenshot at its own pixels — 1280 wide
    // is what a browser tool returns at 1x — with the atlas's gutter.
    assert!(engine.colour_size() >= 1280 + 4);
}

/// The engraved zero is the face's own `zero` alternate — a different
/// glyph at the *same* advance — so the data voice can ask for it by
/// feature and the cell contract (one column per character) is
/// untouched. Both mono cuts carry it; the terminal, which never
/// shapes, keeps the dotted default the build plan chose.
#[test]
fn the_engraved_zero_is_an_alternate_at_the_same_advance() {
    let mut engine = engine();
    let engraved = Features {
        tabular_figures: true,
        slashed_zero: true,
    };
    for font in [EmbeddedFonts::REGULAR, EmbeddedFonts::BOLD] {
        let plain = engine.run("0", font, size(), Features::default());
        let slashed = engine.run("0", font, size(), engraved);
        assert_ne!(
            plain.glyphs[0].glyph_id, slashed.glyphs[0].glyph_id,
            "`zero` engages a different outline"
        );
        assert_eq!(plain.width, slashed.width, "at the same advance");
        assert!(
            !Arc::ptr_eq(&plain, &slashed),
            "features are part of the cache key"
        );
    }
    // The alternate is the zero's alone: every other figure and the
    // letter O shape the same under either setting.
    let plain = engine.run(
        "123456789O",
        EmbeddedFonts::REGULAR,
        size(),
        Features::default(),
    );
    let slashed = engine.run("123456789O", EmbeddedFonts::REGULAR, size(), engraved);
    let ids = |run: &ShapedRun| run.glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>();
    assert_eq!(ids(&plain), ids(&slashed));
}
