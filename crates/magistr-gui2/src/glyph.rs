use qs_gpu::atlas::AtlasEntry;

/// One glyph ready to draw: where its coverage sits in the atlas, and
/// the whole-pixel pen position its subpixel phase was split off from.
///
/// The split is the point. `GlyphKey::quantize` puts the fractional part
/// of the pen position into *which bitmap was rasterized* rather than
/// into the quad's coordinates, so a run's glyphs land on whole pixels
/// and still advance by fractional amounts — which is what stops a row
/// of text from looking blurry while its columns stay where the shaper
/// put them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// The atlas entry: uv on the coverage page, the bitmap's size, and
    /// its offsets from the pen and the baseline.
    pub entry: AtlasEntry,
    /// The integer pen position to draw at.
    pub pen: f32,
}
