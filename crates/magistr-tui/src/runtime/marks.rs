//! The frame's marks as pixel surfaces: every glyph, cap, segment,
//! bar, rule and panel ring the view declared on its tree, drawn once
//! and shown under the text until the mark, its rect or the cell size
//! changes.

use magistr_cockpit::layout::{Mark, Node, Rect, Resolved, marks};
use magistr_cockpit::pixel::{CellSize, Chip, Gauge, Panel, PixelSurface, Rule, Segment};
use magistr_cockpit::render::Color;
use magistr_cockpit::widget_id::WidgetId;
use tiny_skia::PixmapMut;

use crate::render::Screen;

/// Where the marks' compositor slots start: above every id the cockpit
/// hands its widgets, so a mark's placement never collides with a
/// widget's surface.
const MARK_ID_BASE: u32 = 0x7F00_0000;

/// What a cached mark surface was painted from: the mark, the cells it
/// covers and the cell size it was scaled by.
type MarkKey = (Mark, Rect, CellSize);

/// The colours a mark falls back to when a pen left one at the
/// terminal's default: the theme's ground and its ink.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fallback {
    /// The theme's base background.
    pub(crate) ground: (u8, u8, u8),
    /// The theme's body text colour.
    pub(crate) ink: (u8, u8, u8),
}

/// The mark surfaces on screen, keyed by what they were painted from.
///
/// A mark that is on the tree again with the same rect and cell size
/// keeps its surface and its compositor slot, so it costs no bytes. A
/// mark that *changed* over the same cells — a fold that opened, a
/// spinner that turned — repaints into the slot that covered them, so
/// the terminal sees one image replaced rather than one deleted and
/// another placed (and a sixel terminal, which repaints the cells a
/// vanished image leaves, sees nothing vanish). A mark that left frees
/// its slot for the next newcomer, and the compositor deletes its
/// placement because the slot was not shown.
#[derive(Debug, Default)]
pub(crate) struct Marks {
    shown: Vec<(MarkKey, WidgetId, PixelSurface)>,
    free: Vec<WidgetId>,
    minted: u32,
}

impl Marks {
    /// Every mark of `root` as laid out by `resolved`, painted at `cell`
    /// and shown on `screen` — nothing without graphics, where the
    /// view's own glyphs stand.
    pub(crate) fn show(
        &mut self,
        root: &Node,
        resolved: &Resolved,
        cell: CellSize,
        fallback: Fallback,
        screen: &mut Screen,
    ) {
        if !screen.has_graphics() {
            self.shown.clear();
            return;
        }
        let wanted: Vec<(MarkKey, Rect)> = marks(root, resolved)
            .into_iter()
            .filter_map(|(rect, mark)| {
                let rect = covered(&mark, rect)?;
                Some(((mark, rect, cell), rect))
            })
            .collect();
        let mut next = Vec::with_capacity(wanted.len());
        // Two passes, so a mark that kept its key is never robbed of its
        // slot by a changed neighbour over the same cells.
        let mut pending = Vec::new();
        for (key, rect) in wanted {
            match self.shown.iter().position(|(known, ..)| *known == key) {
                Some(at) => {
                    let (_, id, surface) = self.shown.remove(at);
                    next.push((key, id, surface));
                }
                None => pending.push((key, rect)),
            }
        }
        for (key, rect) in pending {
            let same_cells = self
                .shown
                .iter()
                .position(|((_, known, _), ..)| *known == rect);
            let id = match same_cells {
                Some(at) => self.shown.remove(at).1,
                None => self.slot(),
            };
            next.push((key.clone(), id, paint(&key.0, rect, cell, fallback)));
        }
        self.free.extend(self.shown.drain(..).map(|(_, id, _)| id));
        for (_, id, surface) in &mut next {
            screen.surface(*id, surface);
        }
        self.shown = next;
    }

    /// A compositor slot for a new surface: a freed one, else a fresh one.
    fn slot(&mut self) -> WidgetId {
        self.free.pop().unwrap_or_else(|| {
            let id = WidgetId::new(MARK_ID_BASE + self.minted);
            self.minted += 1;
            id
        })
    }
}

/// The cells a mark covers: an icon's cell (or two) on the node's first
/// row, the whole rect for everything else; `None` when the icon's cell
/// is past the node's edge.
fn covered(mark: &Mark, rect: Rect) -> Option<Rect> {
    match mark {
        Mark::Icon { col, cols, .. } => {
            if *col >= rect.width {
                return None;
            }
            let cols = (*cols).min(rect.width - col).max(1);
            Some(Rect::new(rect.x + col, rect.y, cols, 1))
        }
        _ => Some(rect),
    }
}

/// A mark painted into a fresh surface over `rect`.
fn paint(mark: &Mark, rect: Rect, cell: CellSize, fallback: Fallback) -> PixelSurface {
    let rgb = |color: Color, or: (u8, u8, u8)| match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => or,
    };
    match mark {
        Mark::Icon { icon, ink, .. } => {
            let (r, g, b) = rgb(*ink, fallback.ink);
            let ink = tiny_skia::Color::from_rgba8(r, g, b, 255);
            let icon = *icon;
            let mut surface = PixelSurface::with_alpha(rect, cell);
            surface.paint(&|pixmap: &mut PixmapMut<'_>| icon.draw(pixmap, ink));
            surface
        }
        Mark::Chip { fill, ground } => {
            let mut surface = PixelSurface::new(rect, cell);
            surface.paint(&Chip::new(
                rgb(*fill, fallback.ink),
                rgb(*ground, fallback.ground),
            ));
            surface
        }
        Mark::Segment {
            fill,
            track,
            ground,
            first,
            last,
        } => {
            let mut surface = PixelSurface::new(rect, cell);
            surface.paint(
                &Segment::new(rgb(*track, fallback.ink), rgb(*ground, fallback.ground))
                    .fill(fill.map(|fill| rgb(fill, fallback.ground)))
                    .ends(*first, *last),
            );
            surface
        }
        Mark::Bar { fill, line, ground } => {
            let (r, g, b) = rgb(*line, fallback.ink);
            let (gr, gg, gb) = rgb(*ground, fallback.ground);
            let mut surface = PixelSurface::new(rect, cell);
            surface.paint(
                &Gauge::new(f32::from(*fill) / 1000.0)
                    .line(r, g, b)
                    .background(gr, gg, gb),
            );
            surface
        }
        Mark::Rule { ink } => {
            let (r, g, b) = rgb(*ink, fallback.ink);
            let mut surface = PixelSurface::with_alpha(rect, cell);
            surface.paint(&Rule::new(r, g, b));
            surface
        }
        Mark::Panel { border, title } => {
            let (r, g, b) = rgb(*border, fallback.ink);
            let mut surface = PixelSurface::with_alpha(rect, cell);
            surface.paint(&Panel::new(r, g, b, cell).title(*title));
            surface
        }
    }
}

#[cfg(test)]
#[path = "marks.tests.rs"]
mod tests;
