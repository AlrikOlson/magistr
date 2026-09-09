//! One image on screen, as the terminal would hold it.

/// A kitty placement decoded out of a frame's own bytes: where the
/// image sits in cells, how big it is in pixels, its straight-alpha RGBA
/// and the z-index it was placed at.
///
/// The cockpit only ever places at [`Kitty::Z_UNDER_TEXT`], so `z` is
/// carried rather than acted on — it is what a test asserts to prove
/// the compositing order is the protocol's and not a convention this
/// module invented.
///
/// [`Kitty::Z_UNDER_TEXT`]: crate::graphics::Kitty::Z_UNDER_TEXT
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Column of the placement's top-left cell.
    pub col: u16,
    /// Row of the placement's top-left cell.
    pub row: u16,
    /// Cells across (`c=`).
    pub cols: u16,
    /// Cells down (`r=`).
    pub rows: u16,
    /// The image's own width in pixels (`s=`).
    pub width: u32,
    /// The image's own height in pixels (`v=`).
    pub height: u32,
    /// Straight (non-premultiplied) RGBA, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
    /// The z-index the placement was made at (`z=`).
    pub z: i32,
}
