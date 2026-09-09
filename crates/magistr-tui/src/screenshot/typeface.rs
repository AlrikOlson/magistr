//! The embedded monospace faces the screenshot gate draws with.

use std::collections::HashMap;

use fontdue::{Font, FontSettings};

use super::glyph::Glyph;
use magistr_cockpit::fonts::Faces;

/// The three embedded faces, shared with every other renderer — see
/// [`Faces`] for what each is and which licence covers it.
const REGULAR: &[u8] = Faces::REGULAR;
const BOLD: &[u8] = Faces::BOLD;
const FALLBACK: &[u8] = Faces::FALLBACK;

/// The size every face is rasterized at. JetBrains Mono advances 0.6 em,
/// so 15 px is a 9 px advance inside the 10 px cell, and its ascent plus
/// descent (1.32 em) is 19.8 px — inside the 20 px cell, descenders
/// included.
const PX: f32 = 15.0;

/// JetBrains Mono, Regular and Bold, over a DejaVu Sans Mono fallback,
/// rasterized at one fixed size into fixed-size cells — the same cell
/// the pixel layer assumes when the terminal never answered ([`CellSize::DEFAULT`](magistr_cockpit::pixel::CellSize)).
/// Glyphs are cached per `(char, bold)` so a frame pays for each distinct
/// glyph once.
pub struct Typeface {
    regular: Font,
    bold: Font,
    fallback: Font,
    baseline: u32,
    cache: HashMap<(char, bool), Glyph>,
}

impl Typeface {
    /// A cell's width in pixels.
    pub const CELL_WIDTH: u32 = 10;
    /// A cell's height in pixels.
    pub const CELL_HEIGHT: u32 = 20;

    /// Parse the embedded faces. Both are compiled into the binary, so a
    /// parse failure is a broken build, not a runtime condition.
    #[must_use]
    pub fn new() -> Self {
        let settings = FontSettings {
            scale: PX,
            ..FontSettings::default()
        };
        let regular =
            Font::from_bytes(REGULAR, settings).expect("embedded JetBrains Mono Regular parses");
        let bold = Font::from_bytes(BOLD, settings).expect("embedded JetBrains Mono Bold parses");
        let fallback =
            Font::from_bytes(FALLBACK, settings).expect("embedded DejaVu Sans Mono parses");
        // The baseline sits one ascent below the cell's top; a face without
        // horizontal metrics (not this one) would get three quarters of the
        // cell, which is where a 15 px ascent lands anyway.
        let baseline = regular
            .horizontal_line_metrics(PX)
            .map_or(Self::CELL_HEIGHT * 3 / 4, |m| m.ascent.ceil() as u32)
            .min(Self::CELL_HEIGHT);
        Self {
            regular,
            bold,
            fallback,
            baseline,
            cache: HashMap::new(),
        }
    }

    /// Pixels from the cell's top to the baseline glyphs sit on.
    #[must_use]
    pub const fn baseline(&self) -> u32 {
        self.baseline
    }

    /// Whether either embedded face has an outline for `ch`. A `false`
    /// here is a real gap — what the rasterizer draws a tofu box for.
    #[must_use]
    pub fn has_glyph(&self, ch: char) -> bool {
        self.has_own_glyph(ch) || self.fallback.has_glyph(ch)
    }

    /// Whether the primary face — JetBrains Mono — carries the outline
    /// itself (its bold covers the same set). The distinction the
    /// fallback exists for; everything that only needs "is this a gap"
    /// asks [`Self::has_glyph`].
    #[must_use]
    pub fn has_own_glyph(&self, ch: char) -> bool {
        self.regular.has_glyph(ch)
    }

    /// The glyph for `ch`, placed in its cell: the regular or bold
    /// primary face, or the fallback when the primary has no outline
    /// for it. The fallback is rasterized at the same size and sits on
    /// the same baseline, so a symbol lands in the cell its neighbours
    /// are drawn in rather than in one of its own face's making — and
    /// both weights get the same shape, which is what a symbol is.
    pub fn glyph(&mut self, ch: char, bold: bool) -> &Glyph {
        let baseline = self.baseline;
        let face = if !self.has_own_glyph(ch) {
            &self.fallback
        } else if bold {
            &self.bold
        } else {
            &self.regular
        };
        // A fallback glyph is the same bitmap under both keys rather
        // than the cache pretending there are two weights of it.
        self.cache.entry((ch, bold)).or_insert_with(|| {
            let (metrics, coverage) = face.rasterize(ch, PX);
            // fontdue's ymin is the bitmap's bottom edge measured up from
            // the baseline; the bitmap's top row is height above that.
            let bottom = i32::try_from(baseline).unwrap_or(i32::MAX) - metrics.ymin;
            let height = i32::try_from(metrics.height).unwrap_or(i32::MAX);
            Glyph {
                left: metrics.xmin,
                top: bottom - height,
                width: metrics.width,
                height: metrics.height,
                coverage,
            }
        })
    }
}

impl Default for Typeface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "typeface.tests.rs"]
mod tests;
