//! Resolving a [`Node`] tree into rects with taffy's flexbox.

use taffy::compute_leaf_layout;
use taffy::geometry::Size;
use taffy::prelude::{AvailableSpace, NodeId, TaffyTree, length};

use super::basis::Basis;
use super::cell_measure::CellMeasure;
use super::error::LayoutError;
use super::extent::Extent;
use super::measure::Measure;
use super::node::Node;
use super::placed::Placed;
use super::px_rect::PxRect;
use super::rect::Rect;
use super::resolved::Resolved;
use super::text::TextMetrics;

/// The layout engine. Keep one per screen and call [`LayoutTree::compute`]
/// every frame: the engine's node storage is cleared and reused rather than
/// reallocated.
///
/// The root is forced to the screen size; every rect is reported twice —
/// exactly, as the [`PxRect`] the engine produced, and rounded to whole
/// cells — and both are clipped to the parent (the root to the screen),
/// so nothing a layout reports lies outside the buffer. An
/// [`overlay`](Node::overlay) is out of flow, so it is clipped to the
/// screen rather than to its parent — a tooltip anchored inside a
/// one-row footer is not cut off by it — and its own children clip to it
/// as usual.
///
/// # Two units, one tree
///
/// [`LayoutTree::compute`] lays the tree out in cells: [`Basis::CELLS`]
/// and [`CellMeasure`], where every multiplication is by `1.0` and every
/// row is one cell tall. [`LayoutTree::compute_on`] takes a window's
/// basis and its shaper-backed measurer instead, and the same tree comes
/// back measured in pixels with each row as tall as the type it is set
/// in. The cell path is not a special case of the pixel one written in
/// slower arithmetic — it *is* the identity of the pixel one, which is
/// why the terminal cannot drift.
#[derive(Debug)]
pub struct LayoutTree {
    taffy: TaffyTree<usize>,
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutTree {
    /// An engine with no nodes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
        }
    }

    /// Lay `root` out over a `width × height` screen of cells.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] if the engine rejects the tree — which a tree
    /// built from [`Node`]s never triggers.
    pub fn compute(
        &mut self,
        root: &Node,
        width: u16,
        height: u16,
    ) -> Result<Resolved, LayoutError> {
        self.compute_on(
            root,
            f32::from(width),
            f32::from(height),
            Basis::CELLS,
            &mut CellMeasure,
        )
    }

    /// Lay `root` out over a `width × height` viewport measured in
    /// `basis`'s units, sizing text through `measure`.
    ///
    /// # Errors
    ///
    /// See [`LayoutTree::compute`].
    pub fn compute_on(
        &mut self,
        root: &Node,
        width: f32,
        height: f32,
        basis: Basis,
        measure: &mut dyn Measure,
    ) -> Result<Resolved, LayoutError> {
        self.taffy.clear();
        let mut texts: Vec<&TextMetrics> = Vec::new();
        let root_id = self.build(root, basis, measure, &mut texts)?;

        let mut style = root.flex_style(basis);
        style.size = Size {
            width: length(width),
            height: length(height),
        };
        self.taffy.set_style(root_id, style)?;

        let available = Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        };
        self.taffy.compute_layout_with_measure(
            root_id,
            available,
            |inputs, _, context, style| {
                compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        context
                            .and_then(|index| texts.get(*index))
                            .map_or(Size::ZERO, |text| {
                                measure_text(measure, text, known, available)
                            })
                    },
                )
            },
        )?;

        let mut resolved = Resolved::default();
        let screen_px = PxRect::new(0.0, 0.0, width, height);
        let screen = screen_px.to_cells(basis);
        self.collect(
            root,
            root_id,
            Walk::root(screen, screen_px),
            basis,
            &mut resolved,
        )?;
        Ok(resolved)
    }

    /// Build the taffy tree.
    ///
    /// # A cell of *height* on a text leaf is one of its own rows
    ///
    /// A view says `height(Length::Cells(1))` on a row to mean "one
    /// row", and on a terminal that is a cell because a row is a cell.
    /// On a window it is the leaf's own line box: a label row is
    /// shorter than a prose row is shorter than a heading row, and
    /// pinning all three to the mono cell would say the arrangement is
    /// in pixels while every row was still exactly one cell tall. So a
    /// text leaf's vertical unit is [`Measure::line_height`] for its
    /// role — the whole basis, since padding and insets on a text leaf
    /// mean rows too — and on [`Basis::CELLS`] that unit is `1.0`,
    /// which is the basis it already had.
    fn build<'a>(
        &mut self,
        node: &'a Node,
        basis: Basis,
        measure: &mut dyn Measure,
        texts: &mut Vec<&'a TextMetrics>,
    ) -> Result<NodeId, LayoutError> {
        if let Some(text) = node.text_metrics() {
            let rows = Basis::new(basis.x(), measure.line_height());
            texts.push(text);
            return Ok(self
                .taffy
                .new_leaf_with_context(node.flex_style(rows), texts.len() - 1)?);
        }
        let children = node
            .child_nodes()
            .iter()
            .map(|child| self.build(child, basis, measure, texts))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self
            .taffy
            .new_with_children(node.flex_style(basis), &children)?)
    }

    fn collect(
        &self,
        node: &Node,
        id: NodeId,
        walk: Walk,
        basis: Basis,
        out: &mut Resolved,
    ) -> Result<(), LayoutError> {
        let layout = self.taffy.layout(id)?;
        let x = walk.origin.0 + layout.location.x;
        let y = walk.origin.1 + layout.location.y;
        // Out of flow means out of the parent's clip as well as out of
        // its flex line: an overlay answers to the screen.
        let (clip, clip_px) = if node.is_overlay() {
            (walk.screen, walk.screen_px)
        } else {
            (walk.clip, walk.clip_px)
        };
        // The two readings are clipped in their own space rather than
        // one being derived from the other, so the cell rect is the
        // arithmetic the cell-only engine did — rounding first, then
        // intersecting whole cells — and the pixel rect keeps every
        // fraction the engine produced.
        let rect = to_cells(x, y, layout.size.width, layout.size.height, basis).intersection(clip);
        let px = PxRect::new(x, y, layout.size.width, layout.size.height).intersection(clip_px);
        // The scrollable overflow rect's right/bottom are the reachable
        // extent from the scroll origin; floored at the node's own size so
        // `content - rect` is the scroll range even when the content fits.
        let overflow = layout.scrollable_overflow_rect;
        let content = Extent {
            width: cell(
                (overflow.right / basis.x())
                    .ceil()
                    .max(layout.size.width / basis.x()),
            ),
            height: cell(
                (overflow.bottom / basis.y())
                    .ceil()
                    .max(layout.size.height / basis.y()),
            ),
        };
        out.push(Placed {
            id: node.widget_id(),
            depth: walk.depth,
            layer: walk.layer,
            rect,
            px,
            content,
        });
        for (index, child) in node.child_nodes().iter().enumerate() {
            let child_id = self.taffy.child_at_index(id, index)?;
            self.collect(
                child,
                child_id,
                walk.below(child, (x, y), rect, px),
                basis,
                out,
            )?;
        }
        Ok(())
    }
}

/// The state that flows down the collecting walk: where the parent's box
/// begins in engine units, how deep the node is, which overlay layer it
/// sits on, what it is clipped to in each of the two readings, and the
/// screen an overlay answers to instead.
#[derive(Clone, Copy, Debug)]
struct Walk {
    origin: (f32, f32),
    depth: u16,
    layer: u16,
    clip: Rect,
    clip_px: PxRect,
    screen: Rect,
    screen_px: PxRect,
}

impl Walk {
    /// The root's walk: the screen's origin, no depth, no layer, clipped
    /// to the screen.
    const fn root(screen: Rect, screen_px: PxRect) -> Self {
        Self {
            origin: (0.0, 0.0),
            depth: 0,
            layer: 0,
            clip: screen,
            clip_px: screen_px,
            screen,
            screen_px,
        }
    }

    /// The walk for `child`, one level under a parent whose box begins at
    /// `origin` and which landed on `rect`/`px`: a step deeper always, a
    /// layer up when the child is out of flow.
    fn below(self, child: &Node, origin: (f32, f32), rect: Rect, px: PxRect) -> Self {
        Self {
            origin,
            depth: self.depth.saturating_add(1),
            layer: self.layer.saturating_add(u16::from(child.is_overlay())),
            clip: rect,
            clip_px: px,
            screen: self.screen,
            screen_px: self.screen_px,
        }
    }
}

/// Round an engine rect to whole cells, dropping the part left of or above
/// the screen.
fn to_cells(x: f32, y: f32, width: f32, height: f32, basis: Basis) -> Rect {
    let left = (x / basis.x()).round().max(0.0);
    let top = (y / basis.y()).round().max(0.0);
    let right = ((x + width) / basis.x()).round().max(left);
    let bottom = ((y + height) / basis.y()).round().max(top);
    Rect::new(
        cell(left),
        cell(top),
        cell(right - left),
        cell(bottom - top),
    )
}

fn cell(value: f32) -> u16 {
    // Clamped, so the truncating cast is exact for every finite value and
    // NaN becomes 0.
    value.clamp(0.0, f32::from(u16::MAX)) as u16
}

/// Size a text leaf: as wide as it can be within what it is offered, as
/// tall as its rows at that width.
fn measure_text(
    measure: &mut dyn Measure,
    text: &TextMetrics,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    let (max_content, min_content) = measure.intrinsic(text);
    let width = known.width.unwrap_or_else(|| match available.width {
        AvailableSpace::Definite(limit) => max_content.min(limit),
        AvailableSpace::MaxContent => max_content,
        AvailableSpace::MinContent => min_content,
    });
    let height = known.height.unwrap_or_else(|| measure.height(text, width));
    Size { width, height }
}

#[cfg(test)]
#[path = "tree.tests.rs"]
mod tests;
