//! A declarative description of one node in the layout tree.

use taffy::geometry::{Point, Rect as TaffyRect, Size};
use taffy::prelude::{Display, FlexDirection, FlexWrap, Position, Style, length};
use taffy::style::Overflow;

use super::align::Align;
use super::basis::Basis;
use super::frame::Frame;
use super::inset::Inset;
use super::justify::Justify;
use super::length::Length;
use super::mark::Mark;
use super::text::TextMetrics;
use crate::render::Cell;
use crate::render::Style as CellStyle;
use crate::widget_id::WidgetId;

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Row,
    Column,
    Leaf,
    Text(TextMetrics),
    Cells(Vec<Cell>),
}

/// One node of a layout tree: a flex row, a flex column, an empty leaf, a
/// wrapping text leaf or a band of ready-made cells, with the constraints
/// the flex algorithm honours and, optionally, the cell style its subtree
/// is painted in.
///
/// Built by chaining: `Node::row().gap(1).child(Node::leaf().width(Length::Cells(24)))`.
/// Sizes default to `Auto`; `shrink` defaults to `1.0` like CSS, so a
/// fixed-size box in an overfull row gives way unless `shrink(0.0)` is set.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    id: Option<WidgetId>,
    kind: Kind,
    width: Length,
    height: Length,
    min_width: Length,
    min_height: Length,
    max_width: Length,
    max_height: Length,
    basis: Length,
    grow: f32,
    shrink: f32,
    align: Align,
    justify: Justify,
    overlay: Option<Inset>,
    viewport: bool,
    wrap: bool,
    gap: u16,
    padding: u16,
    style: Option<CellStyle>,
    frame: Option<Frame>,
    marks: Vec<Mark>,
    children: Vec<Node>,
}

impl Node {
    fn with_kind(kind: Kind) -> Self {
        Self {
            id: None,
            kind,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            basis: Length::Auto,
            grow: 0.0,
            shrink: 1.0,
            align: Align::default(),
            justify: Justify::default(),
            overlay: None,
            viewport: false,
            wrap: false,
            gap: 0,
            padding: 0,
            style: None,
            frame: None,
            marks: Vec::new(),
            children: Vec::new(),
        }
    }

    /// A container laying its children out left to right.
    #[must_use]
    pub fn row() -> Self {
        Self::with_kind(Kind::Row)
    }

    /// A container laying its children out top to bottom.
    #[must_use]
    pub fn column() -> Self {
        Self::with_kind(Kind::Column)
    }

    /// A childless box sized only by its constraints.
    #[must_use]
    pub fn leaf() -> Self {
        Self::with_kind(Kind::Leaf)
    }

    /// A leaf sized by its text: as wide as the widest line when it fits,
    /// otherwise wrapped to the width it is given, as tall as its rows.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::with_kind(Kind::Text(TextMetrics::new(text)))
    }

    /// A leaf that is one row of cells, each with its own style, exactly
    /// as wide as there are cells.
    ///
    /// The one primitive a text leaf cannot express: a text leaf paints
    /// its whole string in one style, and an image's halfblock row needs
    /// a foreground and a background per cell. Cells past the rect the
    /// layout gives it are clipped.
    #[must_use]
    pub fn cells(cells: Vec<Cell>) -> Self {
        let width = u16::try_from(cells.len()).unwrap_or(u16::MAX);
        Self::with_kind(Kind::Cells(cells))
            .width(Length::Cells(width))
            .height(Length::Cells(1))
            .shrink(0.0)
    }

    /// Attach the id the resolved rect and the hit registry report.
    #[must_use]
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Preferred width.
    #[must_use]
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Preferred height.
    #[must_use]
    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    /// Lower bound on the width.
    #[must_use]
    pub fn min_width(mut self, width: Length) -> Self {
        self.min_width = width;
        self
    }

    /// Lower bound on the height.
    #[must_use]
    pub fn min_height(mut self, height: Length) -> Self {
        self.min_height = height;
        self
    }

    /// Upper bound on the width.
    #[must_use]
    pub fn max_width(mut self, width: Length) -> Self {
        self.max_width = width;
        self
    }

    /// Upper bound on the height.
    #[must_use]
    pub fn max_height(mut self, height: Length) -> Self {
        self.max_height = height;
        self
    }

    /// Share of the parent's spare main-axis space this node takes.
    #[must_use]
    pub fn grow(mut self, grow: f32) -> Self {
        self.grow = grow;
        self
    }

    /// How readily this node gives up space in an overfull parent
    /// (`0.0` never shrinks).
    #[must_use]
    pub fn shrink(mut self, shrink: f32) -> Self {
        self.shrink = shrink;
        self
    }

    /// The main-axis size this node starts from before `grow` and
    /// `shrink` share out the spare space.
    ///
    /// Defaults to `Auto` like CSS, which means "from the content" — so
    /// a `grow(1.0)` node holding wide text still asks for that text's
    /// width first and pushes its siblings. `basis(Length::Cells(0))` is
    /// CSS's `flex: 1 1 0`: the node asks for nothing and takes only its
    /// share of the spare space.
    #[must_use]
    pub const fn basis(mut self, basis: Length) -> Self {
        self.basis = basis;
        self
    }

    /// Where this container puts its children on the cross axis.
    #[must_use]
    pub const fn align_items(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// How this container distributes its children along the main axis —
    /// `Justify::End` right-aligns a row, `Justify::SpaceBetween` pushes
    /// the first and last to the edges.
    #[must_use]
    pub const fn justify_content(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }

    /// Take this node out of flow and position it by `inset` from its
    /// parent's edges: it costs its siblings no space, it paints over
    /// whatever it covers (nodes are painted in tree order, so declare
    /// an overlay after what it covers), and every click inside it
    /// resolves to it rather than to the in-flow nodes underneath — see
    /// [`Placed::layer`](super::Placed::layer).
    ///
    /// An edge left [`Length::Auto`] is unconstrained; setting both
    /// edges of an axis on a node with no size on that axis stretches it
    /// between them. Unlike an in-flow node, an overlay's subtree is
    /// clipped to the **screen** rather than to its parent, so a tooltip
    /// anchored inside a one-row footer is not cut off by it.
    #[must_use]
    pub const fn overlay(mut self, inset: Inset) -> Self {
        self.overlay = Some(inset);
        self
    }

    /// An out-of-flow layer covering its parent with `child` centred on
    /// both axes — a modal.
    ///
    /// Derived from the primitives rather than being one of its own: a
    /// full-bleed [`overlay`](Self::overlay) whose
    /// [`justify_content`](Self::justify_content) and
    /// [`align_items`](Self::align_items) are both centred. `child`
    /// keeps its own size, whatever it is.
    #[must_use]
    pub fn centered_overlay(child: Node) -> Self {
        Self::column()
            .overlay(Inset::all(0))
            .justify_content(Justify::Center)
            .align_items(Align::Center)
            .child(child)
    }

    /// Whether this node is out of flow.
    #[must_use]
    pub const fn is_overlay(&self) -> bool {
        self.overlay.is_some()
    }

    /// A vertically scrolling region: allowed to shrink below its content
    /// height (min-height 0 plus CSS `overflow: scroll` on that axis, so
    /// the flex algorithm's automatic minimum size never resists), its
    /// children clipped to its rect. The resolved layout reports the
    /// content extent so a caller can draw a scrollbar and offset children.
    #[must_use]
    pub fn viewport(mut self) -> Self {
        self.viewport = true;
        self.min_height = Length::Cells(0);
        self
    }

    /// Let children that do not fit on the main axis wrap onto new lines.
    #[must_use]
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// Cells between adjacent children (rows and columns alike).
    #[must_use]
    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// Cells of padding inside every edge, taken out of the node's own size.
    #[must_use]
    pub fn padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// Paint this node's subtree in `style` — the root's fills the screen,
    /// a descendant's replaces it for that subtree, an unstyled node
    /// inherits.
    #[must_use]
    pub fn style(mut self, style: CellStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Wear `frame` as a border: one cell is reserved on every edge by
    /// the layout engine (children are laid out inside the ring), and the
    /// painter draws the rounded box-drawing border there.
    #[must_use]
    pub fn frame(mut self, frame: Frame) -> Self {
        self.frame = Some(frame);
        self
    }

    /// The frame this node wears, if any.
    #[must_use]
    pub const fn frame_spec(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// Ask the pixel layer to draw `mark` over this node's cells — see
    /// [`Mark`]. A node may carry several: a line with a glyph at each
    /// end carries two icons.
    #[must_use]
    pub fn mark(mut self, mark: Mark) -> Self {
        self.marks.push(mark);
        self
    }

    /// The marks this node carries, in the order they were added.
    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Append one child.
    #[must_use]
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// Append several children.
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    /// The node's id, if any.
    #[must_use]
    pub fn widget_id(&self) -> Option<WidgetId> {
        self.id
    }

    /// The node's children in layout order.
    #[must_use]
    pub fn child_nodes(&self) -> &[Node] {
        &self.children
    }

    /// The text this leaf is measured by, if it is a text leaf.
    #[must_use]
    pub fn text_metrics(&self) -> Option<&TextMetrics> {
        match &self.kind {
            Kind::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The ready-made cells this leaf paints, if it is a cell band.
    #[must_use]
    pub fn cell_row(&self) -> Option<&[Cell]> {
        match &self.kind {
            Kind::Cells(cells) => Some(cells),
            _ => None,
        }
    }

    /// Map the styles of the subtree under the node carrying `id`
    /// through `f`, and report whether one was found. Nothing about the
    /// tree's shape changes — this is paint, not layout.
    ///
    /// The found node is given a style even when it carried none: the
    /// one it would have inherited, mapped — so a bare container still
    /// answers. A descendant that carries none keeps inheriting, so the
    /// map never turns a plain container into a band it was not. The
    /// first node carrying `id` in tree order wins, like the layout's
    /// own [`Resolved::rect`](super::Resolved::rect).
    pub fn restyle_at(&mut self, id: WidgetId, f: &dyn Fn(&CellStyle) -> CellStyle) -> bool {
        let inherited = self.style.clone().unwrap_or_default();
        self.restyle_found(id, &inherited, f)
    }

    fn restyle_found(
        &mut self,
        id: WidgetId,
        inherited: &CellStyle,
        f: &dyn Fn(&CellStyle) -> CellStyle,
    ) -> bool {
        if self.id == Some(id) {
            self.style = Some(f(self.style.as_ref().unwrap_or(inherited)));
            for child in &mut self.children {
                child.restyle_below(f);
            }
            return true;
        }
        let inherited = self.style.clone().unwrap_or_else(|| inherited.clone());
        self.children
            .iter_mut()
            .any(|child| child.restyle_found(id, &inherited, f))
    }

    /// Map every style this whole tree carries of its own, leaving the
    /// nodes that merely inherit alone.
    ///
    /// The dim a modal puts on everything behind it: one pass over the
    /// finished tree rather than a predicate threaded through every
    /// surface that draws a pen. Only foregrounds move under the
    /// [`Pen::fade`](crate::theme::Pen::fade) this is called with, so
    /// the bands stay where they are — the session dims and never
    /// blanks.
    pub fn restyle_all(&mut self, f: &dyn Fn(&CellStyle) -> CellStyle) {
        self.restyle_below(f);
    }

    /// Map every style this subtree carries of its own, leaving the
    /// nodes that merely inherit alone.
    fn restyle_below(&mut self, f: &dyn Fn(&CellStyle) -> CellStyle) {
        if let Some(style) = &self.style {
            self.style = Some(f(style));
        }
        for child in &mut self.children {
            child.restyle_below(f);
        }
    }

    /// The cell style this node sets for its subtree, if any.
    #[must_use]
    pub const fn cell_style(&self) -> Option<&CellStyle> {
        self.style.as_ref()
    }

    /// The node's constraints as the flex engine's style, resolved on
    /// `basis`.
    ///
    /// Every cell-denominated slot — the sizes, the flex basis, the
    /// overlay insets, the padding, the gap and the frame ring's border
    /// — takes the basis's x on a horizontal edge and its y on a
    /// vertical one. On [`Basis::CELLS`] every one of them is a
    /// multiplication by `1.0`, which is what the terminal's
    /// byte-identity rests on.
    pub(crate) fn flex_style(&self, basis: Basis) -> Style {
        let flex_direction = match self.kind {
            Kind::Column => FlexDirection::Column,
            _ => FlexDirection::Row,
        };
        // A flex basis is measured along the main axis, which is the
        // one the direction names.
        let main = match flex_direction {
            FlexDirection::Column | FlexDirection::ColumnReverse => basis.y(),
            FlexDirection::Row | FlexDirection::RowReverse => basis.x(),
        };
        let (pad_x, pad_y) = (
            length(basis.across(self.padding)),
            length(basis.down(self.padding)),
        );
        let border = |unit: f32| length(if self.frame.is_some() { unit } else { 0.0 });
        Style {
            display: Display::Flex,
            flex_direction,
            flex_wrap: if self.wrap {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            },
            flex_basis: self.basis.dimension(main),
            flex_grow: self.grow,
            flex_shrink: self.shrink,
            align_items: Some(self.align.taffy()),
            justify_content: Some(self.justify.taffy()),
            position: if self.overlay.is_some() {
                Position::Absolute
            } else {
                Position::Relative
            },
            inset: self.overlay.unwrap_or_default().taffy(basis),
            size: Size {
                width: self.width.dimension(basis.x()),
                height: self.height.dimension(basis.y()),
            },
            min_size: Size {
                width: self.min_width.length_percentage_auto(basis.x()),
                height: self.min_height.length_percentage_auto(basis.y()),
            },
            max_size: Size {
                width: self.max_width.length_percentage_auto(basis.x()),
                height: self.max_height.length_percentage_auto(basis.y()),
            },
            gap: Size {
                width: length(basis.across(self.gap)),
                height: length(basis.down(self.gap)),
            },
            padding: TaffyRect {
                left: pad_x,
                right: pad_x,
                top: pad_y,
                bottom: pad_y,
            },
            border: TaffyRect {
                left: border(basis.x()),
                right: border(basis.x()),
                top: border(basis.y()),
                bottom: border(basis.y()),
            },
            overflow: Point {
                x: Overflow::Visible,
                y: if self.viewport {
                    Overflow::Scroll
                } else {
                    Overflow::Visible
                },
            },
            ..Style::default()
        }
    }
}

#[cfg(test)]
#[path = "node.tests.rs"]
mod tests;
