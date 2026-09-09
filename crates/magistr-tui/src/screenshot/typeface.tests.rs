use super::*;

#[test]
fn the_faces_load_and_the_baseline_sits_inside_the_cell() {
    let face = Typeface::new();
    assert_eq!(Typeface::CELL_WIDTH, 10);
    assert_eq!(Typeface::CELL_HEIGHT, 20);
    assert!(
        (10..=Typeface::CELL_HEIGHT).contains(&face.baseline()),
        "baseline {}",
        face.baseline()
    );
}

#[test]
fn ascii_box_drawing_and_blocks_have_glyphs() {
    let face = Typeface::new();
    let missing: Vec<char> =
        "abcXYZ09 ~!@#$%^&*()[]{}<>/\\|─│┌┐└┘╭╮╰╯├┤┬┴┼═║█▁▂▃▄▅▆▇▀▄▌▐░▒▓▲▼◀▶◂▸›‹•·"
            .chars()
            .filter(|&ch| !face.has_glyph(ch))
            .collect();
    assert!(missing.is_empty(), "missing glyphs: {missing:?}");
}

#[test]
fn a_private_use_code_point_has_no_glyph() {
    let face = Typeface::new();
    assert!(!face.has_glyph('\u{e000}'));
}

#[test]
fn a_glyph_fits_inside_its_cell_descender_included() {
    let mut face = Typeface::new();
    for ch in ['M', 'g', 'q', 'Å'] {
        let glyph = face.glyph(ch, false).clone();
        assert!(glyph.width > 0 && glyph.height > 0, "{ch:?} is empty");
        assert_eq!(glyph.coverage.len(), glyph.width * glyph.height);
        let right = glyph.left + i32::try_from(glyph.width).expect("width");
        let bottom = glyph.top + i32::try_from(glyph.height).expect("height");
        assert!(
            glyph.left >= 0 && right <= 10,
            "{ch:?} x: {}..{right}",
            glyph.left
        );
        assert!(
            glyph.top >= 0 && bottom <= 20,
            "{ch:?} y: {}..{bottom}",
            glyph.top
        );
        // Hairline strokes at 15 px never reach full coverage; a glyph is
        // legible when some pixel is at least four fifths covered.
        assert!(
            glyph.coverage.iter().any(|&c| c >= 204),
            "{ch:?} has a near-solid pixel"
        );
    }
}

#[test]
fn blocks_and_box_drawing_span_the_cells_full_height() {
    // These must meet their neighbours above and below. The guarantee
    // is the synthesizer's, not the face's: `raster::symbol` asks
    // `boxdraw` for the block and box-drawing ranges before any glyph
    // cache, and the synthetic glyph is cut from the cell
    // itself — the bespoke face's own block glyphs stop a pixel short
    // of the cell's top, which is exactly why the ranges are geometry
    // and not typography.
    for ch in ['█', '│', '▐'] {
        let glyph = crate::screenshot::boxdraw::glyph(ch)
            .unwrap_or_else(|| panic!("{ch:?} is the synthesizer's to draw"));
        let bottom = glyph.top + i32::try_from(glyph.height).expect("height");
        assert!(glyph.top <= 0, "{ch:?} starts at the top: {}", glyph.top);
        assert!(bottom >= 20, "{ch:?} reaches the bottom: {bottom}");
    }
}

#[test]
fn a_space_covers_nothing() {
    let mut face = Typeface::new();
    let glyph = face.glyph(' ', false);
    assert!(glyph.coverage.iter().all(|&c| c == 0));
}

#[test]
fn bold_is_its_own_face_and_heavier() {
    let mut face = Typeface::new();
    let regular: u32 = face
        .glyph('M', false)
        .coverage
        .iter()
        .map(|&c| u32::from(c))
        .sum();
    let bold: u32 = face
        .glyph('M', true)
        .coverage
        .iter()
        .map(|&c| u32::from(c))
        .sum();
    assert!(bold > regular, "bold {bold} vs regular {regular}");
}

#[test]
fn glyphs_are_cached_and_deterministic_across_instances() {
    let mut a = Typeface::new();
    let mut b = Typeface::new();
    let first = a.glyph('q', false).clone();
    let again = a.glyph('q', false).clone();
    let other = b.glyph('q', false).clone();
    assert_eq!(first, again);
    assert_eq!(first, other);
}

/// The primary face supplies each required UI symbol with a nonempty outline.
/// The symbol fallback remains available for glyphs outside the primary face.
const ONCE_MISSING: [char; 5] = ['▰', '▱', '★', '⚑', '✦'];

/// A codepoint the primary face genuinely lacks and DejaVu carries — a
/// comet, fittingly — so the fallback path keeps a real exemplar.
const FALLBACK_ONLY: char = '☄';

#[test]
fn the_symbols_jetbrains_mono_lacked_are_the_primary_faces_own() {
    let face = Typeface::new();
    for ch in ONCE_MISSING {
        assert!(face.has_own_glyph(ch), "{ch:?} is the primary's to draw");
    }
    assert!(
        !face.has_own_glyph(FALLBACK_ONLY),
        "the fallback exemplar must not be the primary's"
    );
    assert!(
        face.has_glyph(FALLBACK_ONLY),
        "the fallback exemplar has a glyph somewhere"
    );
}

#[test]
fn a_symbol_glyph_fits_its_cell_and_is_legible() {
    let mut face = Typeface::new();
    for ch in ONCE_MISSING.into_iter().chain([FALLBACK_ONLY]) {
        let glyph = face.glyph(ch, false).clone();
        assert!(glyph.width > 0 && glyph.height > 0, "{ch:?} is empty");
        assert_eq!(glyph.coverage.len(), glyph.width * glyph.height);
        let right = glyph.left + i32::try_from(glyph.width).expect("width");
        let bottom = glyph.top + i32::try_from(glyph.height).expect("height");
        // A pixel of side-bearing overhang is allowed: `blit` clips
        // every glyph pixel to its own cell's span, so an outline that
        // leans a pixel past the box loses that pixel rather than a
        // neighbour's ink.
        assert!(
            glyph.left >= -1 && right <= i32::try_from(Typeface::CELL_WIDTH).expect("cell") + 1,
            "{ch:?} x: {}..{right}",
            glyph.left
        );
        assert!(
            glyph.top >= -1 && bottom <= i32::try_from(Typeface::CELL_HEIGHT).expect("cell") + 1,
            "{ch:?} y: {}..{bottom}",
            glyph.top
        );
        // The five cockpit symbols are chunky enough that some pixel is
        // at least four fifths covered; the comet is all hairline, so
        // the bar for it is only that real ink exists at this size.
        let floor = if ch == FALLBACK_ONLY { 64 } else { 204 };
        assert!(
            glyph.coverage.iter().any(|&c| c >= floor),
            "{ch:?} has ink at least {floor}/255 covered"
        );
    }
}

#[test]
fn the_bars_two_cells_are_not_the_same_glyph() {
    let mut face = Typeface::new();
    let full = face.glyph('▰', false).clone();
    let empty = face.glyph('▱', false).clone();
    let ink = |g: &Glyph| -> u32 { g.coverage.iter().map(|&c| u32::from(c)).sum() };
    assert!(
        ink(&full) > ink(&empty),
        "a filled bar cell is heavier than an empty one: {} vs {}",
        ink(&full),
        ink(&empty)
    );
}

#[test]
fn bold_asks_the_fallback_for_the_same_shape() {
    // The fallback is one face: a symbol it draws is a shape, not a
    // weight, so a bold comet is the comet. (The primary family's own
    // symbols — the star included, now that the bespoke face carries
    // it — do have a real bold.)
    let mut face = Typeface::new();
    let regular = face.glyph(FALLBACK_ONLY, false).clone();
    let bold = face.glyph(FALLBACK_ONLY, true).clone();
    assert_eq!(regular, bold);
}
