use std::sync::Arc;

use qs_gpu::atlas::{AtlasBudget, AtlasEntry, GlyphAtlas, PendingImage, PendingUpload};
use qs_text::cache::ShapedRunCache;
use qs_text::fontdb::{FontDb, FontId};
use qs_text::raster::{GlyphKey, GlyphRaster};
use qs_text::shape::{ShapedRun, Shaper};
use qs_text::{Features, PxSize};

use crate::fonts::EmbeddedFonts;
use crate::glyph::Glyph;
use crate::metrics::CellMetrics;

/// Everything the painter needs to turn a string into glyph quads, and
/// the atlas both tiers read those quads out of.
///
/// One object rather than four fields on the painter, because the four
/// are a unit: a run is shaped at a size, its glyphs are rasterized at
/// that size, and the atlas entry a quad samples is keyed by
/// `(face, glyph, size, subpixel phase)`. Splitting them would let a
/// caller shape at one size and sample at another, which is a class of
/// bug that simply cannot be written here.
///
/// `&mut self` throughout: [`Shaper`] and [`GlyphRaster`] each own a
/// scratch cache and belong to one thread, which is the framework's
/// Constitution II expressed in a signature rather than a comment.
pub struct TextEngine {
    db: Arc<dyn FontDb>,
    shaper: Shaper,
    runs: ShapedRunCache,
    raster: GlyphRaster,
    atlas: GlyphAtlas,
}

impl TextEngine {
    /// Coverage-page edge. 1024² holds every distinct
    /// `(glyph, size, phase)` a cockpit frame draws several times over —
    /// a 120x40 screen is 4800 cells over an alphabet of a few hundred —
    /// and is a quarter of the framework's own default, because a
    /// terminal-shaped UI has no CJK body text to hold.
    const ATLAS: u32 = 1024;
    /// Colour-page edge. Twice the framework's default: the page holds the stream's
    /// images at their own pixels, and a screenshot is 1280 px wide before a Retina
    /// display doubles it, so the default's 1024 would fit no real screenshot at all.
    /// 2048² × 4 is 16 MiB against the atlas's 96 MiB cap, and the CPU tier mirrors it
    /// lazily, on the first picture rather than at construction — a still with no
    /// picture allocates none of it. A picture wider still is admitted fit to the page
    /// ([`ImageQuad::admission`](crate::ImageQuad::admission)).
    const COLOUR: u32 = 2048;
    /// Distinct glyphs admitted per frame.
    ///
    /// The framework rations this because a file manager's corpus is
    /// unbounded; the cockpit's is its own alphabet, so the first frame
    /// wanting every glyph at once is the *normal* case rather than the
    /// pathological one. A bound that made the opening frame converge
    /// over four frames would make a golden a picture of the atlas
    /// filling up.
    const UPLOADS: u32 = 4096;
    /// Shaped runs kept. A cockpit frame is a few hundred rows, and the
    /// rows that change between frames are the streaming ones — so this
    /// holds several screens' worth and the steady-state miss count is
    /// what the turn actually added.
    const RUNS: usize = 4096;

    /// An engine over the embedded faces.
    #[must_use]
    pub fn embedded() -> Self {
        Self::new(Arc::new(EmbeddedFonts::new()))
    }

    /// An engine over `db`.
    #[must_use]
    pub fn new(db: Arc<dyn FontDb>) -> Self {
        Self {
            shaper: Shaper::new(Arc::clone(&db)),
            db,
            runs: ShapedRunCache::new(Self::RUNS),
            raster: GlyphRaster::new(),
            atlas: GlyphAtlas::with_budget(
                AtlasBudget {
                    coverage_size: Self::ATLAS,
                    colour_size: Self::COLOUR,
                },
                Self::UPLOADS,
            ),
        }
    }

    /// The shaper this engine measures and lays runs out with.
    ///
    /// Exposed because two tables are built *from the face* rather than
    /// assumed — the cell grid ([`CellMetrics`]) and the per-role line
    /// boxes ([`TypeMetrics`](crate::TypeMetrics)) — and both are built
    /// once, outside a frame, from the same shaper the frame then uses.
    pub fn shaper(&mut self) -> &mut Shaper {
        &mut self.shaper
    }

    /// The cell grid this engine's mono face makes at `px`.
    #[must_use]
    pub fn metrics(&mut self, px: f32) -> Option<CellMetrics> {
        let font = self.db.mono_font();
        CellMetrics::new(&mut self.shaper, font, px)
    }

    /// Start a frame: the atlas's per-frame bounds and the run cache's
    /// counters reset here and nowhere else.
    pub fn begin_frame(&mut self) {
        self.atlas.begin_frame();
        self.runs.begin_frame();
    }

    /// `text` shaped on `font` at `size` under `features`, from the
    /// cache when it is there.
    ///
    /// The features are part of the cache key: the data voice's
    /// engraved zero and the plain one are different runs, and a
    /// caller that shaped a receipt under one and painted it under the
    /// other would draw glyphs the measure never saw.
    pub fn run(
        &mut self,
        text: &str,
        font: FontId,
        size: PxSize,
        features: Features,
    ) -> Arc<ShapedRun> {
        self.runs
            .get_or_shape(&mut self.shaper, text, font, size, features)
    }

    /// The atlas entry for one shaped glyph at pen position `x`, and the
    /// integer pen position to draw it at.
    ///
    /// `None` is survivable and means one of three things the atlas
    /// counts apart: the glyph has no ink (a space), the per-frame bound
    /// is spent, or the atlas is full. The painter draws nothing and the
    /// row's advance is unaffected, because the advance comes from the
    /// shaped run and never from the bitmap.
    pub fn glyph(&mut self, font: FontId, glyph_id: u16, size: PxSize, x: f32) -> Option<Glyph> {
        let (key, pen) = GlyphKey::quantize(font, glyph_id, size, x);
        let entry = self.atlas.get_or_insert(&*self.db, &mut self.raster, key)?;
        Some(Glyph { entry, pen })
    }

    /// Admit a decoded picture to the colour page under `key`, calling
    /// `decode` only on a miss.
    ///
    /// The signature is the atlas's own, forwarded rather than wrapped,
    /// because the *bound* is the interesting part: the framework admits
    /// at most [`IMAGE_UPLOADS_PER_FRAME`](qs_gpu::atlas::IMAGE_UPLOADS_PER_FRAME)
    /// pictures a frame, so a caller with more surfaces than that has to
    /// build more than one frame to converge — see
    /// [`Frame::converged`](crate::Frame::converged).
    pub fn image(
        &mut self,
        key: qs_gpu::atlas::ImageKey,
        decode: impl FnOnce(qs_gpu::atlas::ImageKey) -> Option<qs_gpu::atlas::RgbaImage>,
    ) -> Option<AtlasEntry> {
        self.atlas.get_or_decode_image(key, decode)
    }

    /// Whether every entry this frame wanted is resident — the question
    /// a headless render asks before it decides the frame it has is the
    /// frame to keep.
    #[must_use]
    pub fn converged(&self) -> bool {
        self.atlas.converged()
    }

    /// The coverage-page edge, for sizing a CPU-tier mirror.
    #[must_use]
    pub fn atlas_size(&self) -> u32 {
        self.atlas.size()
    }

    /// The colour-page edge, which is a different number.
    #[must_use]
    pub fn colour_size(&self) -> u32 {
        self.atlas.colour_size()
    }

    /// Take this frame's coverage uploads for a tier to apply.
    pub fn take_uploads(&mut self) -> Vec<PendingUpload> {
        self.atlas.take_uploads()
    }

    /// Take this frame's colour uploads.
    pub fn take_image_uploads(&mut self) -> Vec<PendingImage> {
        self.atlas.take_image_uploads()
    }
}

impl std::fmt::Debug for TextEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextEngine")
            .field("runs", &self.runs.len())
            .field("atlas", &self.atlas.stats())
            .finish()
    }
}

#[cfg(test)]
#[path = "text.tests.rs"]
mod tests;
