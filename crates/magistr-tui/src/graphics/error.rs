//! The ways a graphics command can be refused before any byte is written.

use std::fmt;

/// A pixel buffer that does not describe the image it claims to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsError {
    /// `pixels.len()` is not `width × height × bytes-per-pixel`.
    PixelCount {
        /// The byte count the dimensions and format require.
        expected: usize,
        /// The byte count that was passed.
        actual: usize,
    },
    /// Width or height is zero — the terminal would reject the transmit.
    EmptyImage,
}

impl fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PixelCount { expected, actual } => write!(
                f,
                "pixel buffer holds {actual} bytes, the image needs {expected}"
            ),
            Self::EmptyImage => f.write_str("image is empty (zero width or height)"),
        }
    }
}

impl std::error::Error for GraphicsError {}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
