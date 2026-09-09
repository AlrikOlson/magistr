/// One named region of a capture the parity diff measures on its own — a header, a
/// feed, a rail — with the share of its unmasked pixels the view may differ by there,
/// read from the same `NN.mask.json` the exclusions come from.
///
/// The frame percentage proved unable to tell a fix from two errors that were
/// cancelling, so the region is the primary signal and the frame threshold survives as
/// the backstop: a residual names a component or an arrangement rather than one number
/// per view.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    /// The component or arrangement the rect belongs to — the word a
    /// failing row prints.
    pub name: String,
    /// Left edge, in capture pixels.
    pub x: u32,
    /// Top edge, in capture pixels.
    pub y: u32,
    /// Width in pixels; always positive.
    pub w: u32,
    /// Height in pixels; always positive.
    pub h: u32,
    /// The share of the region's unmasked pixels it may differ by.
    pub budget: f32,
}

impl Region {
    /// Whether the pixel at `(x, y)` is inside the region — inclusive
    /// at the origin, exclusive at the far edge, like a mask rect.
    #[must_use]
    pub const fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}
