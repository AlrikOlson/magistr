//! Which widget is under a cell.

use crate::layout::{Basis, PxRect, Rect, Resolved};
use crate::widget_id::WidgetId;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    rect: PxRect,
    z: u16,
    id: WidgetId,
}

/// The rects that answer mouse events this frame.
///
/// Register every hittable rect while painting, then [`HitRegistry::hit`]
/// answers the topmost one for a cell: the highest `z` wins, and among
/// equal `z` the rect registered last — so an overlay registered after the
/// dock it covers takes the click, and a child registered after its parent
/// takes precedence inside it.
///
/// # One store, two coordinate systems
///
/// Entries are kept as [`PxRect`]s and a cell rect is widened into one
/// on the way in, so a terminal — where a unit *is* a cell — asks
/// [`HitRegistry::hit`] and gets exactly the containment it always did:
/// a cell's centre is inside `[x, right)` for precisely the integers
/// `Rect::contains` accepted. A window registers the layout's own pixel
/// boxes ([`HitRegistry::register_layout_px`]) and asks
/// [`HitRegistry::hit_at`], because once rows have different heights no
/// cell row can name a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitRegistry {
    entries: Vec<Entry>,
}

impl HitRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget every rect (call at the start of a frame).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Make the cells of `rect` answer for `id` at stacking level `z`.
    /// Empty rects are ignored.
    pub fn register(&mut self, rect: Rect, z: u16, id: WidgetId) {
        self.register_px(cells_to_px(rect), z, id);
    }

    /// Make the pixels of `rect` answer for `id` at stacking level `z`.
    pub fn register_px(&mut self, rect: PxRect, z: u16, id: WidgetId) {
        if rect.is_empty() {
            return;
        }
        self.entries.push(Entry { rect, z, id });
    }

    /// Register every id-carrying rect by overlay layer, preserving paint
    /// order within each layer. Descendants follow their parents, and a
    /// later modal covers earlier graph labels even when those are deeper.
    ///
    /// The stride between layers is the layout's own
    /// [`max_depth`](Resolved::max_depth) plus one — derived from the
    /// tree being registered rather than picked, so a modal at depth 1
    /// cannot lose a click to the depth-9 in-flow leaf it covers no
    /// matter how deep the tree grows.
    pub fn register_layout(&mut self, resolved: &Resolved, base_z: u16) {
        self.register_each(resolved, base_z, |placed| cells_to_px(placed.rect));
    }

    /// The same, over each node's [`px`](crate::layout::Placed::px) box
    /// instead of its cell rect — what a surface whose layout was
    /// computed in pixels registers, since two rows of different
    /// heights can round to the same cell row and a cell registry would
    /// then hand a click to whichever of them was registered last.
    pub fn register_layout_px(&mut self, resolved: &Resolved, base_z: u16) {
        self.register_each(resolved, base_z, |placed| placed.px);
    }

    fn register_each(
        &mut self,
        resolved: &Resolved,
        base_z: u16,
        box_of: impl Fn(&crate::layout::Placed) -> PxRect,
    ) {
        let stride = resolved.max_depth().saturating_add(1);
        for placed in resolved.placed() {
            if let Some(id) = placed.id {
                let z = base_z.saturating_add(placed.layer.saturating_mul(stride));
                self.register_px(box_of(placed), z, id);
            }
        }
    }

    /// The topmost widget under `(col, row)`, if any — the cell's own
    /// centre, which for a registry of whole cells is the cell.
    #[must_use]
    pub fn hit(&self, col: u16, row: u16) -> Option<WidgetId> {
        self.hit_at(f64::from(col) + 0.5, f64::from(row) + 0.5)
    }

    /// The topmost widget under a point in the registry's own units —
    /// physical pixels on a window.
    #[must_use]
    pub fn hit_at(&self, x: f64, y: f64) -> Option<WidgetId> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.rect.contains(x, y))
            .max_by_key(|(index, entry)| (entry.z, *index))
            .map(|(_, entry)| entry.id)
    }

    /// Number of registered rects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A rect of whole cells as the box it covers on [`Basis::CELLS`],
/// where one unit is one cell.
fn cells_to_px(rect: Rect) -> PxRect {
    PxRect::new(
        f32::from(rect.x) * Basis::CELLS.x(),
        f32::from(rect.y) * Basis::CELLS.y(),
        f32::from(rect.width) * Basis::CELLS.x(),
        f32::from(rect.height) * Basis::CELLS.y(),
    )
}

#[cfg(test)]
#[path = "registry.tests.rs"]
mod tests;
