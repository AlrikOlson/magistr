//! Which compositor the probe's protocol chose.

use super::compositor::Compositor;
use super::compression::Compression;
use super::id_space::IdSpace;
use super::sixel_compositor::SixelCompositor;
use crate::graphics::{Kitty, Protocol};
use crate::render::Encoder;
use magistr_cockpit::pixel::PixelSurface;
use magistr_cockpit::widget_id::WidgetId;

/// The pixel path for this session: kitty's id-addressed placements, or
/// sixel's re-emit-at-the-cursor.
///
/// The two compositors share no state — kitty's is all image and
/// placement ids, sixel's has neither — so this is the one place that
/// knows which is in play, and the widget API above it
/// ([`Screen::surface`](crate::render::Screen::surface)) never learns the
/// difference.
#[derive(Debug)]
pub enum Backend {
    /// The kitty graphics protocol.
    Kitty(Compositor),
    /// Sixel.
    Sixel(SixelCompositor),
}

impl Backend {
    /// The backend for `protocol`, packing kitty's pixels with
    /// `compression` and drawing its image ids from `space` (sixel has
    /// neither: it carries its own run-length encoding and addresses no
    /// image by id).
    #[must_use]
    pub fn new(protocol: Protocol, compression: Compression, space: IdSpace) -> Self {
        match protocol {
            Protocol::Kitty(passthrough) => {
                Self::Kitty(Compositor::new(Kitty::new(passthrough), compression, space))
            }
            Protocol::Sixel => Self::Sixel(SixelCompositor::new()),
        }
    }

    /// Show `surface` as widget `id` this frame.
    pub fn show(&mut self, id: WidgetId, surface: &mut PixelSurface) {
        match self {
            Self::Kitty(compositor) => compositor.show(id, surface),
            Self::Sixel(compositor) => compositor.show(id, surface),
        }
    }

    /// Write the frame's graphics, and say whether a surface vacated a
    /// rect whose cells now have to be painted again. Only sixel can:
    /// kitty places under the text, so the cells were never covered.
    pub fn finish(&mut self, encoder: &mut Encoder, out: &mut Vec<u8>) -> bool {
        match self {
            Self::Kitty(compositor) => {
                compositor.finish(encoder, out);
                false
            }
            Self::Sixel(compositor) => compositor.finish(encoder, out),
        }
    }

    /// The display was cleared: place every surface again.
    pub fn forget_placements(&mut self) {
        match self {
            Self::Kitty(compositor) => compositor.forget_placements(),
            Self::Sixel(compositor) => compositor.forget_placements(),
        }
    }

    /// Give the terminal back whatever this session owes it.
    pub fn release(&mut self, out: &mut Vec<u8>) {
        match self {
            Self::Kitty(compositor) => compositor.release(out),
            Self::Sixel(compositor) => compositor.release(out),
        }
    }
}

#[cfg(test)]
#[path = "backend.tests.rs"]
mod tests;
