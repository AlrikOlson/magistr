//! One node's place on screen after layout.

use super::extent::Extent;
use super::px_rect::PxRect;
use super::rect::Rect;
use crate::widget_id::WidgetId;

/// Where a node landed: its id (if it has one), its depth in the tree
/// (the root is `0`), the overlay layer it sits on, its box — in screen
/// cells and in the engine's own units, both already clipped to its
/// parent (or, on an overlay layer, to the screen) — and the extent of
/// its content, larger than the rect when a viewport's children
/// overflow it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The node's id, if the tree gave it one.
    pub id: Option<WidgetId>,
    /// Nesting depth; children are deeper than their parent.
    pub depth: u16,
    /// How many [`overlay`](super::Node::overlay) nodes are on this
    /// node's path from the root, itself included: `0` for everything
    /// in flow, `1` inside a modal, `2` inside a toast over that modal.
    ///
    /// A layer is above every layer below it whatever the depths
    /// involved — see
    /// [`HitRegistry::register_layout`](crate::hit::HitRegistry::register_layout).
    pub layer: u16,
    /// The cells the node covers.
    pub rect: Rect,
    /// The same box in the engine's own units, unrounded — a cell on
    /// [`Basis::CELLS`](super::Basis::CELLS), a physical pixel on a
    /// window's basis. What the native painter, the pixel chrome and
    /// the accessibility bounds read; [`rect`](Self::rect) is what the
    /// cell renderer reads.
    pub px: PxRect,
    /// The cells the node's content covers, overflow included.
    pub content: Extent,
}
