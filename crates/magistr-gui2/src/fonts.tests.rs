use super::*;

/// The mono half of the database is the terminal's own bytes; the
/// proportional half is this crate's. That split is the point: a native
/// still and a terminal still are drawings of the same outlines wherever
/// the cockpit is column-aligned, and the UI voice exists only where the
/// terminal has nothing to compare against.
#[test]
fn the_embedded_faces_are_the_terminal_mono_bytes_and_this_crate_ui_bytes() {
    let db = EmbeddedFonts::new();
    assert_eq!(
        db.face_data(EmbeddedFonts::REGULAR).map(|d| d.bytes),
        Some(Faces::REGULAR)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::BOLD).map(|d| d.bytes),
        Some(Faces::BOLD)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::FALLBACK).map(|d| d.bytes),
        Some(Faces::FALLBACK)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::MONO_ITALIC).map(|d| d.bytes),
        Some(Faces::ITALIC)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::UI_REGULAR).map(|d| d.bytes),
        Some(UiFaces::REGULAR)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::UI_SEMIBOLD).map(|d| d.bytes),
        Some(UiFaces::SEMIBOLD)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::UI_ITALIC).map(|d| d.bytes),
        Some(UiFaces::ITALIC)
    );
    assert_eq!(
        db.face_data(EmbeddedFonts::DISPLAY).map(|d| d.bytes),
        Some(DisplayFace::REGULAR)
    );
    assert!(db.face_data(FontId(8)).is_none(), "there is no ninth face");
}

/// The display face is the third family and the one the design
/// reserves: Magistr Display, proportional, one cut — queried
/// by family at any weight or slope, because a display face with one
/// home has one voice and never a synthesized second.
#[test]
fn the_display_face_is_magistr_display_in_one_cut() {
    let db = EmbeddedFonts::new();
    let display = db.info(EmbeddedFonts::DISPLAY).expect("the serif parses");
    assert_eq!(display.family, "Magistr Display");
    assert!(!display.monospace, "the display voice is proportional");
    for (weight, style) in [
        (FontWeight::REGULAR, FontStyle::Normal),
        (FontWeight::BOLD, FontStyle::Normal),
        (FontWeight::REGULAR, FontStyle::Italic),
    ] {
        for family in ["Magistr Display", "Source Serif 4 Display"] {
            assert_eq!(
                db.query(family, weight, style),
                Some(EmbeddedFonts::DISPLAY)
            );
        }
    }
    // Covered by the tailored subset: a title's letters, its dash and
    // its quotes; not the box drawing, which falls to the one fallback.
    for ch in ['A', 'z', '0', '\u{2014}', '\u{201c}'] {
        assert_eq!(
            db.fallback(ch, EmbeddedFonts::DISPLAY),
            Some(EmbeddedFonts::DISPLAY),
            "{ch:?} should stay on the display face"
        );
    }
    assert_eq!(
        db.fallback('\u{2500}', EmbeddedFonts::DISPLAY),
        Some(EmbeddedFonts::FALLBACK)
    );
}

/// The bespoke mono build keeps the cell contract JetBrains Mono set, read off the
/// embedded bytes themselves: upem 1000, an `M` advancing 600 (0.6 em), and hhea 1020 /
/// −300 / 0 — the numbers `CellMetrics` derives the 9x20 grid and its baseline from,
/// and the numbers `scripts/build-mono-face.sh` pins after every rebuild. A face that
/// drifts from them would silently move every wrap point and grid, so the drift is a
/// failing test instead.
#[test]
fn the_bespoke_mono_keeps_the_cell_contract() {
    for bytes in [Faces::REGULAR, Faces::BOLD, Faces::ITALIC] {
        let face = ttf_parser::Face::parse(bytes, 0).expect("the embedded face parses");
        assert_eq!(face.units_per_em(), 1000);
        assert_eq!(face.ascender(), 1020);
        assert_eq!(face.descender(), -300);
        assert_eq!(face.line_gap(), 0);
        let m = face.glyph_index('M').expect("M has an outline");
        assert_eq!(face.glyph_hor_advance(m), Some(600), "0.6 em advance");
    }
}

#[test]
fn bold_is_its_own_face_and_never_a_synthesized_weight() {
    let db = EmbeddedFonts::new();
    assert_ne!(
        db.face_data(EmbeddedFonts::REGULAR).map(|d| d.bytes),
        db.face_data(EmbeddedFonts::BOLD).map(|d| d.bytes)
    );
    assert_ne!(
        db.face_data(EmbeddedFonts::UI_REGULAR).map(|d| d.bytes),
        db.face_data(EmbeddedFonts::UI_SEMIBOLD).map(|d| d.bytes)
    );
}

/// `ui_font_at` answers the *proportional* family, and its one break is
/// at 600 rather than at `FontWeight::BOLD` — qs-ui's heaviest type role
/// is 600, so a rule keyed on 700 would send every heading to the
/// regular face and the scale's one weight distinction would never reach
/// the screen.
#[test]
fn the_ui_voice_breaks_at_the_weight_the_type_scale_actually_uses() {
    let db = EmbeddedFonts::new();
    assert_eq!(db.ui_font(), EmbeddedFonts::UI_REGULAR);
    assert_eq!(
        db.ui_font_at(FontWeight::REGULAR),
        EmbeddedFonts::UI_REGULAR
    );
    assert_eq!(db.ui_font_at(FontWeight::MEDIUM), EmbeddedFonts::UI_REGULAR);
    assert_eq!(db.ui_font_at(SEMIBOLD), EmbeddedFonts::UI_SEMIBOLD);
    assert_eq!(db.ui_font_at(FontWeight::BOLD), EmbeddedFonts::UI_SEMIBOLD);
    assert_eq!(db.mono_font(), EmbeddedFonts::REGULAR);
}

/// The whole cockpit vocabulary stays on the primary face — including the four symbols
/// JetBrains Mono has no outline for (`▰▱★⚑✦`), which the bespoke
/// Iosevka build covers itself. DejaVu remains the declared fallback for codepoints
/// outside the face — an agent can stream anything — but no glyph the cockpit itself
/// draws reaches it. Same guarantee `Typeface::has_glyph` gives the rasterizer,
/// asserted here against the other parser so the two renderers cannot disagree about
/// which face draws a glyph.
#[test]
fn the_cockpit_vocabulary_never_leaves_the_primary_face() {
    let db = EmbeddedFonts::new();
    for ch in ['▰', '▱', '★', '⚑', '✦', 'a', 'Z', '0', '·', '─', '╭', '▸'] {
        assert_eq!(
            db.fallback(ch, EmbeddedFonts::REGULAR),
            Some(EmbeddedFonts::REGULAR),
            "{ch:?} should stay on the primary face"
        );
    }
}

/// A codepoint no embedded face carries is `None` — the SC-006 signal,
/// not a silent `.notdef`.
#[test]
fn a_codepoint_no_embedded_face_carries_is_reported_rather_than_guessed() {
    let db = EmbeddedFonts::new();
    assert_eq!(db.fallback('\u{10FFFD}', EmbeddedFonts::REGULAR), None);
}

#[test]
fn the_two_voices_report_themselves_as_what_they_are() {
    let db = EmbeddedFonts::new();
    let mono = db.info(EmbeddedFonts::REGULAR).expect("regular parses");
    assert!(mono.family.to_ascii_lowercase().contains("almagest"));
    assert!(mono.monospace, "the data voice is monospaced");
    let ui = db.info(EmbeddedFonts::UI_REGULAR).expect("the sans parses");
    assert!(ui.family.to_ascii_lowercase().contains("hanken"));
    assert!(!ui.monospace, "the UI voice is proportional");
    assert_eq!(db.enumerate().len(), 8);
}

/// Family queries answer the two embedded families and refuse the rest,
/// so a caller cannot silently be given a face it did not ask for.
#[test]
fn a_family_that_is_not_embedded_is_refused_rather_than_approximated() {
    let db = EmbeddedFonts::new();
    assert_eq!(
        db.query("Almagest Mono", FontWeight::BOLD, FontStyle::Normal),
        Some(EmbeddedFonts::BOLD)
    );
    assert_eq!(
        db.query("DejaVu Sans Mono", FontWeight::REGULAR, FontStyle::Normal),
        Some(EmbeddedFonts::FALLBACK)
    );
    assert_eq!(
        db.query("Hanken Grotesk", FontWeight::REGULAR, FontStyle::Normal),
        Some(EmbeddedFonts::UI_REGULAR)
    );
    assert_eq!(
        db.query("Hanken Grotesk", SEMIBOLD, FontStyle::Normal),
        Some(EmbeddedFonts::UI_SEMIBOLD)
    );
    assert_eq!(
        db.query("Hanken Grotesk", FontWeight::REGULAR, FontStyle::Italic),
        Some(EmbeddedFonts::UI_ITALIC)
    );
    assert_eq!(
        db.query("Almagest Mono", FontWeight::REGULAR, FontStyle::Italic),
        Some(EmbeddedFonts::MONO_ITALIC)
    );
    assert_eq!(
        db.query("Helvetica", FontWeight::REGULAR, FontStyle::Normal),
        None
    );
}

/// The proportional family has the same blind spots the mono one does —
/// the geometric shapes and box drawing a UI face has no reason to carry
/// — so the one DejaVu fallback answers for both voices.
#[test]
fn the_ui_voice_falls_back_to_the_same_one_face() {
    let db = EmbeddedFonts::new();
    for ch in ['▰', '▱'] {
        assert_eq!(
            db.fallback(ch, EmbeddedFonts::UI_REGULAR),
            Some(EmbeddedFonts::FALLBACK),
            "{ch:?} should fall back from the UI voice too"
        );
    }
    for ch in ['a', 'Z', '0', '·'] {
        assert_eq!(
            db.fallback(ch, EmbeddedFonts::UI_REGULAR),
            Some(EmbeddedFonts::UI_REGULAR),
            "{ch:?} should stay on the UI face"
        );
    }
}
