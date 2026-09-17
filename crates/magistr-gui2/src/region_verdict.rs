use std::fmt;

/// What a parity comparison measured inside one named region: how many
/// of the region's unmasked pixels differ from the capture, against the
/// region's own budget.
///
/// The row the gate's table prints under a view, so a miss names a
/// component or an arrangement rather than moving a frame-wide number.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionVerdict {
    /// The region's name, from the mask.
    pub name: String,
    /// Unmasked pixels of the region whose colour is outside the
    /// tolerance.
    pub differing: usize,
    /// Pixels of the region the mask left in play.
    pub unmasked: usize,
    /// The share of `unmasked` the region may differ by.
    pub budget: f32,
}

impl RegionVerdict {
    /// The share of the region's unmasked pixels that differ, `0..=1`.
    #[must_use]
    pub fn share(&self) -> f32 {
        if self.unmasked == 0 {
            return 0.0;
        }
        self.differing as f32 / self.unmasked as f32
    }

    /// Whether the region is within its budget.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.share() <= self.budget
    }
}

impl fmt::Display for RegionVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:.2}% of {} px (budget {:.2}%)",
            self.name,
            self.share() * 100.0,
            self.unmasked,
            self.budget * 100.0
        )
    }
}
