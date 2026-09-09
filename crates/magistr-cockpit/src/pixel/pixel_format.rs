//! How the bytes of a packed pixel surface are laid out.

/// The pixel layout of a packed surface: what [`PixelSurface::pack`](super::PixelSurface::pack)
/// produced and what a transport transmits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// Three bytes per pixel — a quarter fewer bytes when there is no
    /// transparency to carry.
    Rgb,
    /// Four bytes per pixel with straight (non-premultiplied) alpha.
    Rgba,
}

impl PixelFormat {
    /// Bytes per pixel.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

#[cfg(test)]
#[path = "pixel_format.tests.rs"]
mod tests;
