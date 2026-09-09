use std::fmt;

use crate::region_verdict::RegionVerdict;

/// What a parity comparison measured: how many of the pixels the mask
/// left in play differ from the capture, against the view's threshold —
/// and the same measurement inside every named region, against each
/// region's own budget.
///
/// A measurement, not a judgement — `passes` is the one place the
/// threshold and the budgets are applied, and `Display` prints the
/// numbers whichever way it went, so the gate's table shows a passing
/// view's distance from its capture as well as a failing one's.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    /// Unmasked pixels whose colour is outside the tolerance.
    pub differing: usize,
    /// Pixels the mask left in play.
    pub unmasked: usize,
    /// The share of `unmasked` a view may differ by and still pass —
    /// the backstop; the regions are the primary signal.
    pub threshold: f32,
    /// The still's size when it is not the capture's — every pixel
    /// counts as differing then, because there is nothing to compare.
    pub size_mismatch: Option<((u32, u32), (u32, u32))>,
    /// One row per named region of the mask, in the mask's order —
    /// empty on a size mismatch, because there was nothing to measure.
    pub regions: Vec<RegionVerdict>,
}

impl Verdict {
    /// The share of unmasked pixels that differ, `0..=1`.
    #[must_use]
    pub fn share(&self) -> f32 {
        if self.unmasked == 0 {
            return 0.0;
        }
        self.differing as f32 / self.unmasked as f32
    }

    /// Whether the view is within its threshold, every region within
    /// its budget — and the still the same size as its capture, which
    /// neither can excuse.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.size_mismatch.is_none()
            && self.share() <= self.threshold
            && self.regions.iter().all(RegionVerdict::passes)
    }

    /// `PASS` or `FAIL`, the word the gate's table prints.
    #[must_use]
    pub fn status(&self) -> &'static str {
        if self.passes() { "PASS" } else { "FAIL" }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(((cw, ch), (sw, sh))) = self.size_mismatch {
            return write!(
                f,
                "the still is {sw}x{sh} and the capture {cw}x{ch}; nothing to compare"
            );
        }
        write!(
            f,
            "{:.2}% of {} unmasked px differ (threshold {:.2}%)",
            self.share() * 100.0,
            self.unmasked,
            self.threshold * 100.0
        )?;
        for region in self.regions.iter().filter(|r| !r.passes()) {
            write!(f, "; {region}")?;
        }
        Ok(())
    }
}
