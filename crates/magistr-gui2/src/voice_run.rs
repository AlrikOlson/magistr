use std::borrow::Cow;
use std::sync::Arc;

use qs_text::shape::{ShapedGlyph, ShapedRun};

use crate::type_set::TypeSet;

/// One run set in a voice: the text as the volume writes it, the face
/// and size it was shaped at, its shaped glyphs, and the tracking laid
/// over them.
///
/// # Why this exists rather than two call sites agreeing
///
/// Tracking and uppercase happen *outside* the shaper — CSS applies
/// `text-transform` before shaping and `letter-spacing` after it — so
/// each is a step a caller could forget. There are two callers that
/// must never disagree: [`ShapedMeasure`](crate::ShapedMeasure), which
/// tells the layout engine how wide a run is and where a paragraph
/// breaks, and [`Painter`](crate::Painter), which draws it. Two
/// wrappers that agreed today would drift the first time one grew a
/// rule; there is one, and both ask it.
///
/// # Why tracking is counted in clusters
///
/// CSS adds letter-spacing after every *character*, so a ligature or a
/// mark stack takes one gap and not one per glyph. [`ShapedGlyph`]
/// carries the byte offset of its grapheme cluster, so the gap count is
/// the number of distinct clusters the run shaped into — and the
/// trailing gap is included, because the browser includes it and a
/// tracked cap's box is wider than its ink for exactly that reason.
pub struct VoiceRun<'a> {
    /// The text as painted: uppercased when the voice asks, borrowed
    /// from the caller's slice when it does not.
    pub text: Cow<'a, str>,
    /// The face and size it was shaped at.
    pub set: TypeSet,
    /// The shaped glyphs, untracked — the shaper's own answer.
    pub run: Arc<ShapedRun>,
    /// Pixels added after every grapheme cluster.
    pub tracking: f32,
}

impl VoiceRun<'_> {
    /// How wide this run is on screen, tracking included.
    #[must_use]
    pub fn width(&self) -> f32 {
        if self.tracking == 0.0 {
            return self.run.width;
        }
        #[allow(clippy::cast_precision_loss)]
        let clusters = self.clusters() as f32;
        self.run.width + self.tracking * clusters
    }

    /// How many grapheme clusters the run shaped into.
    #[must_use]
    pub fn clusters(&self) -> usize {
        let mut count = 0;
        let mut last: Option<u32> = None;
        for glyph in &self.run.glyphs {
            if last != Some(glyph.cluster) {
                count += 1;
                last = Some(glyph.cluster);
            }
        }
        count
    }

    /// Each glyph with its pen position within the run, tracking laid
    /// over the shaper's own advances.
    ///
    /// Untracked voices — every one but the cap and the badge — hand
    /// back the shaper's `x` unchanged, so the ordinary run is the code
    /// that ran before tracking existed.
    pub fn placed(&self) -> impl Iterator<Item = (&ShapedGlyph, f32)> + '_ {
        let tracking = self.tracking;
        let mut gaps = 0u32;
        let mut last: Option<u32> = None;
        self.run.glyphs.iter().map(move |glyph| {
            if last != Some(glyph.cluster) {
                if last.is_some() {
                    gaps += 1;
                }
                last = Some(glyph.cluster);
            }
            (
                glyph,
                glyph.x + tracking * f32::from(u16::try_from(gaps).unwrap_or(u16::MAX)),
            )
        })
    }
}

#[cfg(test)]
#[path = "voice_run.tests.rs"]
mod tests;
