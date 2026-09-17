//! Ships pixel surfaces to the terminal as sixel images.
//!
//! Everything the kitty [`Compositor`](super::Compositor) does with ids —
//! store once, place many, delete one placement — is unavailable here: a
//! sixel image has no identity on the terminal side. So a surface reaches
//! the screen exactly one way, by putting the cursor at its top-left cell
//! and emitting the whole DCS string, and leaves it by having the cells
//! underneath painted again.
//!
//! Two consequences shape this file. Encoding is cached per surface, so a
//! surface that only *moved* costs one cursor move and a memcpy rather
//! than a re-quantisation. And because a sixel image covers the cells it
//! sits on (kitty places under the text at `z=-1`; sixel has no z), a
//! surface that moves or leaves the frame vacates a rect whose cells are
//! now stale. [`SixelCompositor::prepare`] reports those rects before the
//! cell diff is emitted; [`SixelCompositor::finish`] then restores images
//! touched by those cell writes, in their original drawing order.

use std::collections::HashMap;

use crate::graphics::sixel;
use crate::render::{Change, Encoder};
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::PixelSurface;
use magistr_cockpit::widget_id::WidgetId;

/// Save the cursor (DECSC) — emitting a sixel moves it.
const SAVE_CURSOR: &[u8] = b"\x1b7";
/// Restore the cursor (DECRC), so the encoder's idea of it stays true.
const RESTORE_CURSOR: &[u8] = b"\x1b8";

/// Keeps one encoded image per surface and re-emits it when the surface
/// is dirty, has moved, or has not been on screen yet.
#[derive(Debug, Default)]
pub struct SixelCompositor {
    slots: HashMap<WidgetId, Slot>,
    packed: Vec<u8>,
    queue: Vec<Pending>,
    /// Rects surfaces moved away from this frame.
    vacated: Vec<Rect>,
}

/// One surface's encoded image and where it is on the terminal.
#[derive(Clone, Debug, Default)]
struct Slot {
    /// The last sixel string encoded for this surface; empty until one is.
    image: Vec<u8>,
    /// Where the image is on the terminal, if it is.
    placed: Option<Rect>,
    /// Shown this frame.
    seen: bool,
}

/// An image to write at the end of the frame.
#[derive(Clone, Copy, Debug)]
struct Pending {
    id: WidgetId,
    rect: Rect,
    dirty: bool,
}

impl SixelCompositor {
    /// A compositor with nothing on screen.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Show `surface` as widget `id` this frame: re-encodes it when it is
    /// dirty (and marks it clean), queues an emission when it is dirty,
    /// moved or not yet on screen. An empty rect counts as not shown.
    pub fn show(&mut self, id: WidgetId, surface: &mut PixelSurface) {
        let rect = surface.rect();
        if rect.is_empty() {
            return;
        }
        let stale = surface.is_dirty() || !self.slots.contains_key(&id);
        let mut image = None;
        if stale {
            let format = surface.pack(&mut self.packed);
            let mut encoded = Vec::new();
            let written = sixel::encode(
                &mut encoded,
                format,
                surface.width(),
                surface.height(),
                &self.packed,
            );
            if written.is_ok() {
                surface.clean();
                image = Some(encoded);
            }
        }
        let slot = self.slots.entry(id).or_default();
        slot.seen = true;
        let encoded = image.is_some();
        if let Some(image) = image {
            slot.image = image;
        }
        if slot.image.is_empty() {
            return;
        }
        let moved = slot.placed != Some(rect);
        if moved && let Some(previous) = slot.placed {
            self.vacated.push(previous);
        }
        // Retain drawing order even for cached images: cell repair or an
        // earlier image can damage an unchanged image above it.
        self.queue.push(Pending {
            id,
            rect,
            dirty: encoded || moved,
        });
    }

    /// Take the rects that need cell repair before this frame's images.
    /// Call after all surfaces have been shown, before encoding cell changes.
    pub fn prepare(&mut self) -> Vec<Rect> {
        let mut vacated = std::mem::take(&mut self.vacated);
        for slot in self.slots.values_mut() {
            if !slot.seen
                && let Some(rect) = slot.placed.take()
            {
                vacated.push(rect);
            }
        }
        vacated
    }

    /// Write dirty images and restore cached images touched by cell writes
    /// or earlier images. DECSC/DECRC preserves the encoder's cursor.
    pub fn finish(&mut self, encoder: &mut Encoder, out: &mut Vec<u8>, changes: &[Change<'_>]) {
        let mut emitted: Vec<Rect> = Vec::new();
        for pending in self.queue.drain(..) {
            let rect = pending.rect;
            let damaged = changes.iter().any(|change| {
                !rect
                    .intersection(Rect::new(change.col, change.row, change.cell.width(), 1))
                    .is_empty()
            }) || emitted
                .iter()
                .any(|other| !rect.intersection(*other).is_empty());
            if !pending.dirty && !damaged {
                continue;
            }
            let Some(slot) = self.slots.get_mut(&pending.id) else {
                continue;
            };
            encoder.move_to(rect.x, rect.y, out);
            out.extend_from_slice(SAVE_CURSOR);
            out.extend_from_slice(&slot.image);
            out.extend_from_slice(RESTORE_CURSOR);
            slot.placed = Some(rect);
            emitted.push(rect);
        }
        for slot in self.slots.values_mut() {
            slot.seen = false;
        }
    }

    /// The display was cleared: every surface shown next frame is emitted
    /// again. Nothing was vacated — the clear already repainted the cells.
    pub fn forget_placements(&mut self) {
        for slot in self.slots.values_mut() {
            slot.placed = None;
        }
        self.vacated.clear();
    }

    /// Leaving the session. A sixel terminal stores nothing to delete, so
    /// this only forgets what was on screen; the caller is leaving the
    /// alternate screen, which takes the pixels with it.
    pub fn release(&mut self, _out: &mut Vec<u8>) {
        self.slots.clear();
        self.queue.clear();
        self.vacated.clear();
    }
}

#[cfg(test)]
#[path = "sixel_compositor.tests.rs"]
mod tests;
