use tiny_skia::{Pixmap, PremultipliedColorU8};

use crate::mask::Mask;
use crate::medium::Medium;
use crate::region_verdict::RegionVerdict;
use crate::verdict::Verdict;

/// The pixel comparison behind the mock parity gate: a native still
/// against the blessed capture of the same view in the same medium,
/// under the view's mask.
///
/// A diff and not a review, because a review is a judgement and a diff
/// is a measurement. Two rules make the measurement honest about what
/// it is comparing — a browser's rasterizer against tiny-skia's —
/// without making it soft. (1) A pixel is *the same* when every channel
/// is within [`Parity::TOLERANCE`] of the capture's, which is the
/// anti-aliasing allowance every visual-regression tool carries
/// (pixelmatch's YIQ threshold, odiff's `antialiasing`): two
/// rasterizers put a glyph's edge coverage a few counts apart, and a
/// gate that counted those would measure the rasterizers rather than
/// the design. It is a per-*channel* delta of 24 on 255 — a shade of
/// the same colour, never a different colour. (2) The share that
/// decides is over the pixels the mask left in play, so masking a
/// region never makes the rest count for less.
pub struct Parity;

impl Parity {
    /// The per-channel delta (on 0..=255) inside which two pixels are
    /// the same pixel.
    pub const TOLERANCE: u8 = 24;

    /// Measure `still` against `capture` under `mask` for `medium`.
    #[must_use]
    pub fn compare(capture: &Pixmap, still: &Pixmap, mask: &Mask, medium: Medium) -> Verdict {
        let capture_size = (capture.width(), capture.height());
        let still_size = (still.width(), still.height());
        if capture_size != still_size {
            return Verdict {
                differing: capture.pixels().len(),
                unmasked: capture.pixels().len(),
                threshold: mask.threshold(),
                size_mismatch: Some((capture_size, still_size)),
                regions: Vec::new(),
            };
        }
        let (mut differing, mut unmasked) = (0, 0);
        let mut tallies = vec![(0usize, 0usize); mask.regions().len()];
        for (i, (a, b)) in capture.pixels().iter().zip(still.pixels()).enumerate() {
            let (x, y) = Self::at(capture.width(), i);
            if mask.covers(x, y, medium) {
                continue;
            }
            unmasked += 1;
            let differs = !Self::same(*a, *b);
            if differs {
                differing += 1;
            }
            for (region, tally) in mask.regions().iter().zip(&mut tallies) {
                if region.contains(x, y) {
                    tally.1 += 1;
                    if differs {
                        tally.0 += 1;
                    }
                }
            }
        }
        let regions = mask
            .regions()
            .iter()
            .zip(tallies)
            .map(|(region, (differing, unmasked))| RegionVerdict {
                name: region.name.clone(),
                differing,
                unmasked,
                budget: region.budget,
            })
            .collect();
        Verdict {
            differing,
            unmasked,
            threshold: mask.threshold(),
            size_mismatch: None,
            regions,
        }
    }

    /// The review image for a comparison: the capture dimmed to grey,
    /// masked pixels tinted blue, and every differing pixel red — the
    /// convention every diff tool since pixelmatch has used, so the
    /// picture reads without a legend.
    ///
    /// The image is the capture's size; when the still's size differs
    /// the whole image is red, which is what "nothing to compare" looks
    /// like.
    #[must_use]
    pub fn diff(capture: &Pixmap, still: &Pixmap, mask: &Mask, medium: Medium) -> Pixmap {
        let (width, height) = (capture.width(), capture.height());
        let mut out =
            Pixmap::new(width.max(1), height.max(1)).expect("a pixmap with both sides allocates");
        let same_size = still.width() == width && still.height() == height;
        let red = PremultipliedColorU8::from_rgba(255, 40, 40, 255).expect("opaque");
        for (i, (out, a)) in out
            .pixels_mut()
            .iter_mut()
            .zip(capture.pixels())
            .enumerate()
        {
            let (x, y) = Self::at(width, i);
            let grey = Self::luma(*a) / 3 + 40;
            *out = if mask.covers(x, y, medium) {
                PremultipliedColorU8::from_rgba(grey / 2, grey / 2, grey.saturating_add(60), 255)
                    .expect("opaque")
            } else if !same_size || !Self::same(*a, still.pixels()[i]) {
                red
            } else {
                PremultipliedColorU8::from_rgba(grey, grey, grey, 255).expect("opaque")
            };
        }
        out
    }

    /// Whether two pixels are the same pixel under
    /// [`Parity::TOLERANCE`].
    #[must_use]
    pub fn same(a: PremultipliedColorU8, b: PremultipliedColorU8) -> bool {
        let close = |p: u8, q: u8| p.abs_diff(q) <= Self::TOLERANCE;
        close(a.red(), b.red())
            && close(a.green(), b.green())
            && close(a.blue(), b.blue())
            && close(a.alpha(), b.alpha())
    }

    /// The pixel index `i` of a `width`-wide image as `(x, y)`.
    const fn at(width: u32, i: usize) -> (u32, u32) {
        let width = width as usize;
        ((i % width) as u32, (i / width) as u32)
    }

    /// Rec. 601 luma of an opaque pixel, on 0..=255.
    fn luma(p: PremultipliedColorU8) -> u8 {
        let luma =
            0.299 * f32::from(p.red()) + 0.587 * f32::from(p.green()) + 0.114 * f32::from(p.blue());
        luma.round().clamp(0.0, 255.0) as u8
    }
}

#[cfg(test)]
#[path = "parity.tests.rs"]
mod tests;
