//! Ships pixel surfaces to the terminal as kitty placements.

use std::collections::HashMap;

use super::compression::Compression;
use super::id_space::IdSpace;
use crate::graphics::{ImageId, Kitty, PlacementId};
use crate::render::Encoder;
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::PixelSurface;
use magistr_cockpit::widget_id::WidgetId;

/// Keeps one slot per surface — two kitty image ids and one placement id,
/// all drawn from this session's [`IdSpace`] so two cockpits behind one
/// terminal image store (tmux panes under passthrough) never transmit
/// over each other's images — and turns a frame's surfaces into the
/// fewest graphics commands:
///
/// * a dirty surface transmits its pixels under the slot's *idle* id,
///   places that id under the placement id and deletes the previous id's
///   placement, so the screen never lacks an image;
/// * a surface that only moved is placed again under the same ids, which
///   the protocol defines as a replacement;
/// * an unchanged surface costs nothing;
/// * a surface that was placed but not shown this frame has its placement
///   deleted — exactly once — while its image data stays for a comeback.
///
/// Transmits are staged as they arrive; placements are written at
/// [`Compositor::finish`], after the cell diff and before ESU, each preceded
/// by the cursor move the encoder tracks.
#[derive(Debug)]
pub struct Compositor {
    kitty: Kitty,
    compression: Compression,
    space: IdSpace,
    slots: HashMap<WidgetId, Slot>,
    allocated: u32,
    staging: Vec<u8>,
    packed: Vec<u8>,
    payload: Vec<u8>,
    queue: Vec<Pending>,
}

/// One surface's terminal-side identity and what is on screen for it.
#[derive(Clone, Copy, Debug)]
struct Slot {
    ids: [ImageId; 2],
    placement: PlacementId,
    /// Which of `ids` holds the latest pixels.
    current: usize,
    /// Where the placement is on the terminal, if it is.
    placed: Option<Rect>,
    /// Shown this frame.
    seen: bool,
}

impl Slot {
    /// The `n`-th slot of `space`: its image ids `2n+1`, `2n+2` and
    /// placement id `n+1`.
    fn nth(space: IdSpace, n: u32) -> Self {
        Self {
            ids: space.image_ids(n),
            placement: space.placement(n),
            // Nothing is stored yet; the first transmit goes to the idle
            // id, which this makes the first one.
            current: 1,
            placed: None,
            seen: false,
        }
    }
}

/// A placement to write at the end of the frame.
#[derive(Clone, Copy, Debug)]
struct Pending {
    id: WidgetId,
    rect: Rect,
    /// The id whose placement this one supersedes.
    delete_old: Option<ImageId>,
}

impl Compositor {
    /// A compositor writing through `kitty`, packing pixels with
    /// `compression`, its image ids drawn from `space`.
    #[must_use]
    pub fn new(kitty: Kitty, compression: Compression, space: IdSpace) -> Self {
        Self {
            kitty,
            compression,
            space,
            slots: HashMap::new(),
            allocated: 0,
            staging: Vec::new(),
            packed: Vec::new(),
            payload: Vec::new(),
            queue: Vec::new(),
        }
    }

    /// How transmitted pixels are packed.
    #[must_use]
    pub const fn compression(&self) -> Compression {
        self.compression
    }

    /// Show `surface` as widget `id` this frame: stages a transmit when it
    /// is dirty (and marks it clean), queues a placement when it is dirty,
    /// moved or not yet on screen. An empty rect counts as not shown.
    pub fn show(&mut self, id: WidgetId, surface: &mut PixelSurface) {
        let rect = surface.rect();
        let slot = self.slots.entry(id).or_insert_with(|| {
            let slot = Slot::nth(self.space, self.allocated);
            self.allocated += 1;
            slot
        });
        if rect.is_empty() {
            return;
        }
        slot.seen = true;
        let mut delete_old = None;
        let mut place = slot.placed != Some(rect);
        if surface.is_dirty() {
            let idle = 1 - slot.current;
            let format = surface.pack(&mut self.packed);
            let (width, height) = (surface.width(), surface.height());
            let sent = if self.compression.apply(&self.packed, &mut self.payload) {
                self.kitty.transmit_zlib(
                    &mut self.staging,
                    slot.ids[idle],
                    format,
                    width,
                    height,
                    &self.payload,
                )
            } else {
                self.kitty.transmit(
                    &mut self.staging,
                    slot.ids[idle],
                    format,
                    width,
                    height,
                    &self.payload,
                )
            };
            if sent.is_ok() {
                delete_old = slot.placed.map(|_| slot.ids[slot.current]);
                slot.current = idle;
                place = true;
                surface.clean();
            }
        }
        if place {
            self.queue.push(Pending {
                id,
                rect,
                delete_old,
            });
        }
    }

    /// Write the frame's graphics: the staged transmits, then every queued
    /// placement (cursor move, place, delete of the superseded placement),
    /// then one placement delete per surface that left the frame.
    pub fn finish(&mut self, encoder: &mut Encoder, out: &mut Vec<u8>) {
        out.append(&mut self.staging);
        for pending in self.queue.drain(..) {
            let Some(slot) = self.slots.get_mut(&pending.id) else {
                continue;
            };
            let rect = pending.rect;
            encoder.move_to(rect.x, rect.y, out);
            self.kitty.place(
                out,
                slot.ids[slot.current],
                slot.placement,
                rect.width,
                rect.height,
                Kitty::Z_UNDER_TEXT,
            );
            if let Some(old) = pending.delete_old {
                self.kitty.delete_placement(out, old, slot.placement);
            }
            slot.placed = Some(rect);
        }
        for slot in self.slots.values_mut() {
            if !slot.seen && slot.placed.take().is_some() {
                self.kitty
                    .delete_placement(out, slot.ids[slot.current], slot.placement);
            }
            slot.seen = false;
        }
    }

    /// The display was cleared: every surface shown next frame is placed
    /// again (its image data is still stored, so nothing is retransmitted).
    pub fn forget_placements(&mut self) {
        for slot in self.slots.values_mut() {
            slot.placed = None;
        }
    }

    /// Delete every image (`d=I`, data included) and forget every slot —
    /// what leaving the session owes the terminal.
    pub fn release(&mut self, out: &mut Vec<u8>) {
        let mut slots: Vec<Slot> = self.slots.drain().map(|(_, slot)| slot).collect();
        slots.sort_by_key(|slot| slot.placement);
        for slot in slots {
            for id in slot.ids {
                self.kitty.delete_image(out, id);
            }
        }
        self.allocated = 0;
        self.staging.clear();
        self.queue.clear();
    }
}

#[cfg(test)]
#[path = "compositor.tests.rs"]
mod tests;
