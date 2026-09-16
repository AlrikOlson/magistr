//! The double-buffered screen: draw into the back buffer, present the diff.

use std::collections::HashMap;

use super::diff::diff;
use super::encode::Encoder;
use crate::graphics::Protocol;
use crate::pixel::{Backend, Compression, IdSpace};
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::PixelSurface;
use magistr_cockpit::render::Buffer;
use magistr_cockpit::render::Headline;
use magistr_cockpit::widget_id::WidgetId;

/// Two [`Buffer`]s, an [`Encoder`] and, once the probe found a terminal
/// that speaks a graphics protocol, a [`Backend`] for pixel surfaces.
///
/// [`Screen::buffer`] hands out the back buffer, blank at the start of every
/// frame; [`Screen::surface`] shows a pixel surface in the frame;
/// [`Screen::present`] diffs the cells against what is on the terminal and
/// appends the encoded frame — cell changes, then the surfaces' transmits,
/// placements and deletes, then whatever [`Screen::enclose`] queued — to a
/// `Vec<u8>` inside one BSU/ESU bracket, and
/// makes the back buffer the front buffer. The first frame after
/// [`Screen::new`] or [`Screen::resize`] clears the display, redraws
/// everything and places every surface again.
///
/// A sixel surface covers the cells it sits on, so when one moves or
/// leaves the frame the backend says so and the next frame is a full
/// redraw — the only way to get those cells back.
#[derive(Debug)]
pub struct Screen {
    front: Buffer,
    back: Buffer,
    encoder: Encoder,
    redraw: bool,
    graphics: Option<Backend>,
    enclosed: Vec<u8>,
    headline: Option<Headline>,
    shown: Option<Headline>,
    occlusion: Option<Rect>,
    masked: HashMap<WidgetId, (Rect, PixelSurface)>,
    pixels: super::pixel_frame::PixelFrame,
}

impl Screen {
    /// A screen of `width × height` cells whose first frame redraws fully.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            front: Buffer::new(width, height),
            back: Buffer::new(width, height),
            encoder: Encoder::new(),
            redraw: true,
            graphics: None,
            enclosed: Vec::new(),
            headline: None,
            shown: None,
            occlusion: None,
            masked: HashMap::new(),
            pixels: super::pixel_frame::PixelFrame::default(),
        }
    }

    /// Composite pixel surfaces through `protocol` from now on, with
    /// kitty image ids drawn from `space` — this session's own, so a
    /// cockpit sharing the terminal's image store with another never
    /// transmits over its images.
    pub fn enable_graphics(
        &mut self,
        protocol: Protocol,
        compression: Compression,
        space: IdSpace,
    ) {
        self.graphics = Some(Backend::new(protocol, compression, space));
    }

    /// Emit `SGR 4:x` underline styles and `SGR 58` underline colours
    /// from now on — for a terminal
    /// [`Env::styled_underlines`](crate::graphics::Env::styled_underlines)
    /// vouches for. Off by default; every other terminal is sent the
    /// plain `SGR 4` for a styled rule.
    pub const fn enable_underline_styles(&mut self) {
        self.encoder.enable_underline_styles();
    }

    /// Send `bytes` inside the next frame's bracket, after the cells and
    /// the pixels and before ESU — terminal state that belongs to the
    /// frame rather than to the grid, the way a pixel placement does. The
    /// cockpit's OSC 9;4 progress report is the one caller.
    ///
    /// Whatever is queued goes out with the next [`Screen::present`] and
    /// is then forgotten, so a frame nobody enclosed anything for costs
    /// nothing.
    pub fn enclose(&mut self, bytes: &[u8]) {
        self.enclosed.extend_from_slice(bytes);
    }

    /// Show `headline` — a run of scaled text through kitty's text
    /// sizing protocol — in the frame being drawn, over the rows the
    /// view left blank for it.
    ///
    /// Declared per frame like a pixel surface and, like one, emitted
    /// only when it *changed*: a settled headline costs no bytes. The
    /// run goes out after the cells and the surfaces, in its own pen,
    /// and the pen is reset before ESU.
    ///
    /// A headline that leaves the frame takes the next frame's full
    /// redraw with it, the way a departing sixel surface does: our
    /// buffer holds blanks where the terminal holds the scaled block, so
    /// a diff that happened to agree with those blanks would leave the
    /// glyphs on screen.
    pub fn headline(&mut self, headline: Option<Headline>) {
        self.headline = headline;
    }

    /// Whether pixel surfaces reach the terminal.
    #[must_use]
    pub const fn has_graphics(&self) -> bool {
        self.graphics.is_some()
    }

    /// Show `surface` as widget `id` in the frame being drawn. Without
    /// graphics this does nothing — the widget draws its text fallback
    /// into the buffer instead.
    pub fn surface(&mut self, id: WidgetId, surface: &mut PixelSurface) {
        let covered = self
            .occlusion
            .map(|rect| surface.rect().intersection(rect))
            .filter(|rect| !rect.is_empty());
        if let Some(covered) = covered {
            let repaint = surface.is_dirty()
                || self.masked.get(&id).is_none_or(|(known, masked)| {
                    *known != covered
                        || masked.rect() != surface.rect()
                        || masked.cell_size() != surface.cell_size()
                });
            if repaint {
                let rect = surface.rect();
                let cell = surface.cell_size();
                let mut masked = PixelSurface::with_alpha(rect, cell);
                masked.paint(&|pixels: &mut tiny_skia::PixmapMut<'_>| {
                    let width = pixels.width() as usize;
                    pixels.data_mut().copy_from_slice(surface.pixels());
                    let x = usize::from(covered.x - rect.x) * usize::from(cell.width);
                    let y = usize::from(covered.y - rect.y) * usize::from(cell.height);
                    let w = usize::from(covered.width) * usize::from(cell.width);
                    let h = usize::from(covered.height) * usize::from(cell.height);
                    for row in y..y + h {
                        pixels.data_mut()[(row * width + x) * 4..(row * width + x + w) * 4].fill(0);
                    }
                });
                self.masked.insert(id, (covered, masked));
                surface.clean();
            }
            if let (Some(graphics), Some((_, masked))) =
                (&mut self.graphics, self.masked.get_mut(&id))
            {
                if self.pixels.is_active() {
                    self.pixels.blend(masked);
                } else {
                    graphics.show(id, masked);
                }
            }
        } else {
            if self.masked.remove(&id).is_some() {
                // The terminal holds the masked copy; restore the original pixels once.
                surface.paint(&|_: &mut tiny_skia::PixmapMut<'_>| {});
            }
            self.surface_unmasked(id, surface);
        }
    }

    /// Opaque cell overlay bounds. A changed cover redraws sixel's retained pixels.
    pub(crate) fn occlude(&mut self, rect: Option<Rect>) {
        if self.occlusion != rect {
            self.redraw = true;
            self.occlusion = rect;
        }
    }

    pub(crate) const fn occlusion(&self) -> Option<Rect> {
        self.occlusion
    }

    /// The overlay's own border remains visible over its cell background.
    pub(crate) fn surface_unmasked(&mut self, id: WidgetId, surface: &mut PixelSurface) {
        if self.pixels.is_active() {
            self.pixels.blend(surface);
        } else if let Some(graphics) = &mut self.graphics {
            graphics.show(id, surface);
        }
    }

    /// Start a graphics frame. All following surfaces are composed into it.
    pub(crate) fn begin_pixels(&mut self, ground: tiny_skia::Pixmap) {
        self.pixels.begin(ground);
    }

    /// Add pixel text and submit changed strips; no UI glyphs reach the cell encoder.
    pub(crate) fn finish_pixels(
        &mut self,
        ink: &tiny_skia::Pixmap,
        cell: magistr_cockpit::pixel::CellSize,
    ) {
        if let Some(graphics) = &mut self.graphics {
            self.pixels
                .finish(ink, self.back.width(), self.back.height(), cell, graphics);
        }
        self.back.fill(&magistr_cockpit::render::Cell::blank());
    }

    /// Delete every image the terminal stores for this screen (before
    /// leaving the session). Graphics stay enabled; the next frame
    /// transmits every surface it shows again.
    pub fn release(&mut self, out: &mut Vec<u8>) {
        if let Some(graphics) = &mut self.graphics {
            graphics.release(out);
        }
    }

    /// `(width, height)` in cells.
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        (self.back.width(), self.back.height())
    }

    /// The back buffer for the frame being drawn.
    pub fn buffer(&mut self) -> &mut Buffer {
        &mut self.back
    }

    /// Resize both buffers (contents dropped); the next frame clears and
    /// redraws the display.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.front.resize(width, height);
        self.back.resize(width, height);
        self.encoder.reset();
        self.redraw = true;
        self.shown = None;
        self.forget_placements();
    }

    /// Forget what is on the terminal so the next frame clears and redraws
    /// everything (after another writer touched the terminal).
    pub fn invalidate(&mut self) {
        self.redraw = true;
        self.encoder.reset();
        self.shown = None;
        self.forget_placements();
    }

    fn forget_placements(&mut self) {
        if let Some(graphics) = &mut self.graphics {
            graphics.forget_placements();
        }
    }

    /// Encode the frame in the back buffer into `out` and start a new,
    /// blank frame. Exactly one BSU/ESU pair is appended per call.
    pub fn present(&mut self, out: &mut Vec<u8>) {
        if self.redraw {
            self.front.resize(self.back.width(), self.back.height());
        }
        let changes = diff(&self.front, &self.back);
        self.encoder
            .begin_frame(self.back.width(), self.redraw, &changes, out);
        let vacated = match &mut self.graphics {
            Some(graphics) => graphics.finish(&mut self.encoder, out),
            None => false,
        };
        let left = self.write_headline(out);
        out.append(&mut self.enclosed);
        Encoder::end_frame(out);
        self.redraw = false;
        std::mem::swap(&mut self.front, &mut self.back);
        self.back.fill(&magistr_cockpit::render::Cell::blank());
        if vacated || left {
            // A surface or a headline left cells holding what it drew;
            // nothing short of a redraw puts the text back under where
            // it was.
            self.invalidate();
        }
    }

    /// Emit this frame's headline when it differs from what the
    /// terminal is showing, or when the frame redrew the display over
    /// it. Returns whether a headline that *was* on screen is gone, so
    /// the caller can take the redraw its cells need.
    fn write_headline(&mut self, out: &mut Vec<u8>) -> bool {
        let changed = self.headline != self.shown || (self.redraw && self.headline.is_some());
        let left = self.headline.is_none() && self.shown.is_some();
        if changed && let Some(headline) = &self.headline {
            let rect = headline.rect();
            self.encoder.move_to(rect.x, rect.y, out);
            self.encoder.set_pen(headline.style(), out);
            self.encoder.headline(headline, out);
            self.encoder.reset_pen(out);
            // The run printed text of its own: where the cursor ended up
            // is the terminal's business, not the encoder's.
            self.encoder.reset();
        }
        self.shown = self.headline.take();
        left
    }
}

#[cfg(test)]
#[path = "screen.tests.rs"]
mod tests;
