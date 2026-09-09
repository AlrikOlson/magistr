//! One rasterized glyph, placed inside its cell.

/// A glyph's coverage bitmap and where it sits in the cell: `left`/`top`
/// are the bitmap's offset from the cell's top-left corner in pixels (a
/// negative `left` means the outline leans past the cell's edge and is
/// clipped), `coverage` is `width × height` bytes, row-major from the top,
/// 0 for none and 255 for a fully covered pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// Pixels from the cell's left edge to the bitmap's left edge.
    pub left: i32,
    /// Pixels from the cell's top edge to the bitmap's top row.
    pub top: i32,
    /// Bitmap width in pixels.
    pub width: usize,
    /// Bitmap height in pixels.
    pub height: usize,
    /// `width × height` coverage bytes, top row first.
    pub coverage: Vec<u8>,
}
