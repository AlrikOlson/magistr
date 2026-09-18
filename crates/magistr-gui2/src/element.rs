use accesskit::Role;
use magistr_cockpit::turn::StreamImage;
use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element_id::ElementId;
use crate::elevation::Elevation;
use crate::mark::Mark;
use crate::pane_tile::PaneTile;
use crate::text_run::TextRun;
use crate::voice::Voice;

/// A declarative node in the native pixel layout tree, rebuilt each frame.
///
/// Padding and gaps use floating-point pixel tokens. [`Voice`] supplies text metrics;
/// radius and [`Elevation`] define the surface. Native layout does not quantize these
/// values to the cockpit's terminal grid.
///
/// The node holds layout, appearance, [`ElementId`], interaction state, and
/// accessibility metadata. `element_layout` places it, `element_state` processes
/// cockpit messages, `element_emit` draws it, and `element_access` publishes its
/// accessibility node.
pub struct Element {
    pub(crate) kind: Kind,
    pub(crate) axis: Axis,
    /// Left, top, right, bottom — in pixels, as the volume writes them.
    pub(crate) pad: [f32; 4],
    pub(crate) gap: f32,
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
    /// A ceiling on the width — CSS `max-width`, the volume's measure
    /// on a column that otherwise grows.
    pub(crate) max_width: Option<f32>,
    /// A floor on the height — CSS `min-height`. Flex min-size defaults
    /// to auto (an item never shrinks below its content), and `0.0`
    /// here is how the feed chain gives that floor up so the window can
    /// hold a turn taller than itself.
    pub(crate) min_height: Option<f32>,
    /// A floor on the width — CSS `min-width`. `0.0` lets a run inside
    /// a row shrink below its single-line width and wrap; a positive
    /// floor is a box that is at least this wide and grows past it with
    /// its content, the way a browser box with `width` and
    /// `min-width:auto` does.
    pub(crate) min_width: Option<f32>,
    pub(crate) grow: f32,
    /// CSS `align-self: stretch` — this element takes its container's
    /// whole cross axis whatever its own content measures, which is
    /// what makes the turn column a MEASURE rather than a box around
    /// its widest row.
    pub(crate) stretch: bool,
    /// CSS `flex-shrink` — 1.0, the web's own default, so a squeezed
    /// run still compresses and clips the way the volume's chips do.
    /// A transcript row sets 0.0: it overflows rather than compacts.
    pub(crate) shrink: f32,
    pub(crate) align: Align,
    pub(crate) justify: Justify,
    /// The rest fill for a surface, and the ink for a text run.
    pub(crate) fill: Option<Srgba>,
    pub(crate) hover_fill: Option<Srgba>,
    pub(crate) press_fill: Option<Srgba>,
    /// A vertical two-stop ramp — the raised primary's lit face.
    pub(crate) gradient: Option<[Srgba; 2]>,
    pub(crate) stroke: Option<(Srgba, f32)>,
    pub(crate) dashed: bool,
    /// The dash and its gap, when the outline breaks — the slot's 4/4
    /// unless a star ring asks for its own rhythm.
    pub(crate) dash: (f32, f32),
    pub(crate) glow: Option<(Srgba, f32)>,
    pub(crate) radius: f32,
    pub(crate) elevation: Elevation,
    pub(crate) id: Option<ElementId>,
    pub(crate) focusable: bool,
    /// One of a run of siblings the arrow keys walk — a feed card's
    /// head. ↑/↓ move focus between walkable elements while one holds
    /// it; nothing else about the element changes.
    pub(crate) walkable: bool,
    pub(crate) disabled: bool,
    pub(crate) focus_ring: Option<Srgba>,
    pub(crate) role: Role,
    pub(crate) label: Option<String>,
    /// Left, top, right, bottom insets from the parent's border box —
    /// any `Some` lifts the element out of flex flow (CSS absolute).
    pub(crate) pin: [Option<f32>; 4],
    /// A text run that never breaks — CSS `white-space:nowrap`, which
    /// the volume puts on every chip. A squeezed run clips; it does
    /// not become two rows.
    pub(crate) nowrap: bool,
    /// Insertion byte offset for a text editor, owned by the shared draft.
    pub(crate) editor_cursor: Option<usize>,
    /// A row that breaks its children into lines — CSS
    /// `flex-wrap: wrap`, the inline box a line of word runs needs so a
    /// marked word can sit inside a sentence that still wraps.
    pub(crate) wrap: bool,
    /// The component a text run belongs to — stamped by [`Element::claim`], read by
    /// [`Element::runs`]; `None` on a box, and on a run nothing has claimed yet.
    pub(crate) owner: Option<Component>,
    /// Painted at its layout coordinates rather than snapped to the
    /// device grid — the browser's treatment of SVG geometry and
    /// transformed layers, which a star's ring and dot are (figure 1's
    /// `<circle>`s); every CSS box snaps ([`Element::exact`]).
    pub(crate) exact: bool,
    pub(crate) children: Vec<Element>,
}

/// What the node is: a box, a run of set type, one of the Almagest
/// set's marks drawn at its own pixel size, a decoded picture drawn
/// into its box, or a program's terminal as a grid of tiles.
pub(crate) enum Kind {
    Surface,
    Text {
        voice: Voice,
        content: String,
    },
    Mark {
        mark: Mark<'static>,
        px: u32,
        ink: (u8, u8, u8),
    },
    Picture {
        image: StreamImage,
    },
    /// A terminal's rows at the mono grid the face makes at `px`: one advance per
    /// column, one line height per row. Every colour is already resolved on the tile;
    /// the box is `cols × advance` by `rows × height`.
    Grid {
        rows: Vec<Vec<PaneTile>>,
        px: f32,
    },
}

/// Which way children stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    Row,
    Column,
}

/// Cross-axis placement of children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
    Stretch,
}

/// Main-axis distribution of children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
}

impl Element {
    fn new(kind: Kind, axis: Axis, role: Role) -> Self {
        Self {
            kind,
            axis,
            pad: [0.0; 4],
            gap: 0.0,
            width: None,
            height: None,
            max_width: None,
            min_height: None,
            min_width: None,
            grow: 0.0,
            stretch: false,
            shrink: 1.0,
            align: Align::Stretch,
            justify: Justify::Start,
            fill: None,
            hover_fill: None,
            press_fill: None,
            gradient: None,
            stroke: None,
            dashed: false,
            dash: (4.0, 4.0),
            glow: None,
            radius: 0.0,
            elevation: Elevation::canvas(),
            id: None,
            focusable: false,
            walkable: false,
            disabled: false,
            focus_ring: None,
            role,
            label: None,
            pin: [None; 4],
            nowrap: false,
            editor_cursor: None,
            wrap: false,
            owner: None,
            exact: false,
            children: Vec::new(),
        }
    }

    /// Paint this box at its layout coordinates, never snapped to the
    /// device grid. The browser pixel-snaps every CSS box's edges but
    /// draws SVG geometry and transformed layers where they fall, with
    /// anti-aliasing — so a star's ring, an SVG `<circle>` in the
    /// volume, keeps its fractional centre while a badge's ring, a CSS
    /// border, lands on whole rows.
    #[must_use]
    pub fn exact(mut self) -> Self {
        self.exact = true;
        self
    }

    /// A box stacking its children top to bottom.
    #[must_use]
    pub fn surface() -> Self {
        Self::new(Kind::Surface, Axis::Column, Role::GenericContainer)
    }

    /// A box laying its children left to right.
    #[must_use]
    pub fn row() -> Self {
        Self::new(Kind::Surface, Axis::Row, Role::GenericContainer)
    }

    /// A run of `content` set in `voice`, drawn in `ink`.
    ///
    /// The ink rides the fill slot — a text run has no ground of its
    /// own, so the one colour field means "what the ink is" here and
    /// "what the surface is" on a box.
    #[must_use]
    pub fn text(voice: Voice, ink: Srgba, content: impl Into<String>) -> Self {
        let mut this = Self::new(
            Kind::Text {
                voice,
                content: content.into(),
            },
            Axis::Row,
            Role::Label,
        );
        this.fill = Some(ink);
        this
    }

    /// One of the Almagest set's marks, drawn `px` square in `ink` —
    /// the same rasterize-and-admit route [`Marks`](crate::Marks)
    /// takes, so a mark on an element is never resampled.
    ///
    /// The ink is bytes rather than a float color because the raster is
    /// keyed by it in the atlas: two inks a rounding error apart must
    /// be one picture, not two.
    #[must_use]
    pub fn mark(mark: Mark<'static>, px: u32, ink: (u8, u8, u8)) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let side = px as f32;
        let mut this = Self::new(Kind::Mark { mark, px, ink }, Axis::Row, Role::Image);
        this.width = Some(side);
        this.height = Some(side);
        this
    }

    /// A decoded picture, `height` pixels tall with its aspect kept and
    /// never scaled up — a golden on the deliverables shelf at thumbnail
    /// height. The box is the picture's own measure, so the row it sits
    /// in wraps around real widths; the picture is admitted to the
    /// colour page the way the feed's pictures are and drawn into the
    /// box. Its alt text is the accessibility label.
    #[must_use]
    pub fn picture(image: StreamImage, height: f32) -> Self {
        let (w, h) = (f32::from(image.width()), f32::from(image.height()));
        let scale = if h > 0.0 { (height / h).min(1.0) } else { 1.0 };
        let label = image.alt().to_owned();
        let mut this = Self::new(Kind::Picture { image }, Axis::Row, Role::Image);
        this.width = Some((w * scale).round().max(1.0));
        this.height = Some((h * scale).round().max(1.0));
        this.label = Some(label);
        this
    }

    /// A program's terminal, `rows` of resolved tiles drawn at the mono grid the face
    /// makes at `px` — the pane a running card opens into. Read-only by construction:
    /// nothing here takes an id or focus, so no key can reach the pty through it. The
    /// rows, as text, are what a screen reader is told.
    #[must_use]
    pub fn grid(rows: Vec<Vec<PaneTile>>, px: f32) -> Self {
        let label = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|tile| tile.text.as_str())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut this = Self::new(Kind::Grid { rows, px }, Axis::Row, Role::Terminal);
        this.label = Some(label);
        this
    }

    /// Add `child` under this element, after the ones already there.
    #[must_use]
    pub fn child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    /// This text run never breaks into rows — CSS `white-space:nowrap`.
    /// A run squeezed under its shaped width clips at the box instead
    /// of wrapping, which is what every single-line control wants.
    #[must_use]
    pub fn nowrap(mut self) -> Self {
        self.nowrap = true;
        self
    }

    /// Render this text as a bounded editor with a proportional insertion caret.
    #[must_use]
    pub fn editor_cursor(mut self, cursor: usize) -> Self {
        self.editor_cursor = Some(cursor);
        self
    }

    pub(crate) fn preview_editor(
        &mut self,
        id: ElementId,
        draft: &str,
        cursor: usize,
        preedit: &str,
    ) {
        if self.id == Some(id)
            && let Kind::Text { content, .. } = &mut self.kind
        {
            *content = draft.to_owned();
            content.insert_str(cursor, preedit);
            self.editor_cursor = Some(cursor + preedit.len());
        }
        for child in &mut self.children {
            child.preview_editor(id, draft, cursor, preedit);
        }
    }

    /// This row breaks its children into lines when they overflow —
    /// CSS `flex-wrap: wrap`. A line of word runs wears it so the line
    /// wraps between words the way an inline box does.
    #[must_use]
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// Pin this element `px` from the parent's left edge, out of flex
    /// flow — CSS absolute positioning, which is how the volume places
    /// a switch's thumb, a meter's fill and a theme card's badge.
    #[must_use]
    pub fn pin_left(mut self, px: f32) -> Self {
        self.pin[0] = Some(px);
        self
    }

    /// Pin this element `px` from the parent's top edge.
    #[must_use]
    pub fn pin_top(mut self, px: f32) -> Self {
        self.pin[1] = Some(px);
        self
    }

    /// Pin this element `px` from the parent's right edge.
    #[must_use]
    pub fn pin_right(mut self, px: f32) -> Self {
        self.pin[2] = Some(px);
        self
    }

    /// Pin this element `px` from the parent's bottom edge.
    #[must_use]
    pub fn pin_bottom(mut self, px: f32) -> Self {
        self.pin[3] = Some(px);
        self
    }

    /// The same padding on all four sides.
    #[must_use]
    pub fn pad(mut self, px: f32) -> Self {
        self.pad = [px; 4];
        self
    }

    /// Horizontal padding — left and right.
    #[must_use]
    pub fn pad_x(mut self, px: f32) -> Self {
        self.pad[0] = px;
        self.pad[2] = px;
        self
    }

    /// Vertical padding — top and bottom.
    #[must_use]
    pub fn pad_y(mut self, px: f32) -> Self {
        self.pad[1] = px;
        self.pad[3] = px;
        self
    }

    /// The bottom padding alone — for the one row of a list the volume
    /// gives a deeper foot than the rest.
    #[must_use]
    pub fn pad_bottom(mut self, px: f32) -> Self {
        self.pad[3] = px;
        self
    }

    /// Pixels between adjacent children.
    #[must_use]
    pub fn gap(mut self, px: f32) -> Self {
        self.gap = px;
        self
    }

    /// A fixed width in pixels.
    #[must_use]
    pub fn width(mut self, px: f32) -> Self {
        self.width = Some(px);
        self
    }

    /// A fixed height in pixels.
    #[must_use]
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }

    /// A ceiling on the width — CSS `max-width`: the element still
    /// grows and shrinks, but never past `px`. The volume's measure on
    /// the turn column (`TURN_MAX_WIDTH`).
    #[must_use]
    pub fn max_width(mut self, px: f32) -> Self {
        self.max_width = Some(px);
        self
    }

    /// A floor on the height — CSS `min-height`. `0.0` matters most:
    /// it removes flex's automatic minimum, which is what lets the feed
    /// chain shrink below its content so the tail can anchor.
    #[must_use]
    pub fn min_height(mut self, px: f32) -> Self {
        self.min_height = Some(px);
        self
    }

    /// A floor on the width — CSS `min-width`. `0.0` removes flex's
    /// automatic minimum on the main axis, which is what lets a run in
    /// a row shrink below its single-line width and wrap; a positive
    /// floor is a box at least this wide that still grows with its
    /// content (the receipt row's key column).
    #[must_use]
    pub fn min_width(mut self, px: f32) -> Self {
        self.min_width = Some(px);
        self
    }

    /// How much of the leftover main-axis space this element takes.
    #[must_use]
    pub fn grow(mut self, share: f32) -> Self {
        self.grow = share;
        self
    }

    /// This element fills its container's cross axis regardless of what
    /// its content measures — CSS `align-self: stretch`. The turn
    /// column wears it so its width is the volume's `max-width`
    /// measure and not its widest row: a box that shrank to its content
    /// would re-wrap the content that sized it.
    #[must_use]
    pub fn stretch(mut self) -> Self {
        self.stretch = true;
        self
    }

    /// This element never compresses when its container overfills —
    /// CSS `flex-shrink: 0`, the transcript row's contract: a row
    /// overflows the feed's box rather than compacting inside it.
    #[must_use]
    pub fn no_shrink(mut self) -> Self {
        self.shrink = 0.0;
        self
    }

    /// Children sit at the cross-axis start instead of stretching.
    #[must_use]
    pub fn align_start(mut self) -> Self {
        self.align = Align::Start;
        self
    }

    /// Children are centred on the cross axis.
    #[must_use]
    pub fn center(mut self) -> Self {
        self.align = Align::Center;
        self
    }

    /// Children sit at the cross-axis end.
    #[must_use]
    pub fn align_end(mut self) -> Self {
        self.align = Align::End;
        self
    }

    /// Children are centred on the main axis.
    #[must_use]
    pub fn justify_center(mut self) -> Self {
        self.justify = Justify::Center;
        self
    }

    /// Children are packed to the main-axis end.
    #[must_use]
    pub fn justify_end(mut self) -> Self {
        self.justify = Justify::End;
        self
    }

    /// The leftover main-axis space opens between the children.
    #[must_use]
    pub fn space_between(mut self) -> Self {
        self.justify = Justify::SpaceBetween;
        self
    }

    /// The rest fill for a surface — see [`Element::text`] for what the
    /// same slot means on a run.
    #[must_use]
    pub fn fill(mut self, color: Srgba) -> Self {
        self.fill = Some(color);
        self
    }

    /// The fill while the pointer is over this element.
    #[must_use]
    pub fn hover_fill(mut self, color: Srgba) -> Self {
        self.hover_fill = Some(color);
        self
    }

    /// The fill while a press is held on this element.
    #[must_use]
    pub fn press_fill(mut self, color: Srgba) -> Self {
        self.press_fill = Some(color);
        self
    }

    /// A vertical ramp from `top` to `bottom` instead of a flat fill.
    #[must_use]
    pub fn gradient(mut self, top: Srgba, bottom: Srgba) -> Self {
        self.gradient = Some([top, bottom]);
        self
    }

    /// An outline of `width` pixels in `color`.
    #[must_use]
    pub fn stroke(mut self, color: Srgba, width: f32) -> Self {
        self.stroke = Some((color, width));
        self
    }

    /// The outline breaks into dashes — the pending star's ring.
    #[must_use]
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    /// The dash rhythm, when the default 4/4 is not the volume's — a
    /// star ring dashes `3 3`.
    #[must_use]
    pub fn dash(mut self, len: f32, gap: f32) -> Self {
        self.dash = (len, gap);
        self
    }

    /// A halo of `color` fading over `falloff` pixels.
    #[must_use]
    pub fn glow(mut self, color: Srgba, falloff: f32) -> Self {
        self.glow = Some((color, falloff));
        self
    }

    /// Corner rounding in pixels — the token table's own numbers.
    #[must_use]
    pub fn radius(mut self, px: f32) -> Self {
        self.radius = px;
        self
    }

    /// How far off the canvas this surface stands.
    #[must_use]
    pub fn elevation(mut self, elevation: Elevation) -> Self {
        self.elevation = elevation;
        self
    }

    /// The identity interaction state is kept under.
    #[must_use]
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// This element can hold keyboard focus.
    #[must_use]
    pub fn focusable(mut self) -> Self {
        self.focusable = true;
        self
    }

    /// The arrow keys walk focus between this element and the other
    /// walkable ones, in tree order — the feed's cards under ↑/↓.
    /// Implies focusable.
    #[must_use]
    pub fn walkable(mut self) -> Self {
        self.focusable = true;
        self.walkable = true;
        self
    }

    /// This element answers no pointer and takes no focus.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    /// The ring drawn around this element while it holds focus.
    #[must_use]
    pub fn focus_ring(mut self, color: Srgba) -> Self {
        self.focus_ring = Some(color);
        self
    }

    /// What this element is, to assistive technology.
    #[must_use]
    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// What this element is called, to assistive technology.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Every text run under this element that no component has claimed yet now belongs
    /// to `component` — the stamp a component's constructor puts on what it built, and
    /// a composition puts on a run it sets itself. An inner component's earlier claim
    /// stands: the key cap inside a command row is the key hint's, not the row's.
    #[must_use]
    pub fn claim(mut self, component: Component) -> Self {
        fn stamp(element: &mut Element, component: Component) {
            if matches!(element.kind, Kind::Text { .. }) && element.owner.is_none() {
                element.owner = Some(component);
            }
            for child in &mut element.children {
                stamp(child, component);
            }
        }
        stamp(&mut self, component);
        self
    }

    /// Every text run in the tree, depth-first, with the component it
    /// belongs to — what the copy-law sweep reads.
    #[must_use]
    pub fn runs(&self) -> Vec<TextRun> {
        let mut runs = Vec::new();
        self.each(&mut |_, element| {
            if let Kind::Text { content, .. } = &element.kind {
                runs.push(TextRun {
                    text: content.clone(),
                    owner: element.owner,
                });
            }
        });
        runs
    }

    /// Visit every element depth-first, handing each its tree index —
    /// the one numbering layout, state, emission and access all agree
    /// on, because they all get it from here.
    pub(crate) fn each<'a>(&'a self, f: &mut impl FnMut(usize, &'a Element)) {
        fn walk<'a>(
            element: &'a Element,
            next: &mut usize,
            f: &mut impl FnMut(usize, &'a Element),
        ) {
            let index = *next;
            *next += 1;
            f(index, element);
            for child in &element.children {
                walk(child, next, f);
            }
        }
        let mut next = 0;
        walk(self, &mut next, f);
    }

    /// How many elements the tree holds, this one included — with
    /// [`emit`](crate::element_emit::emit)'s DFS `range`, `0..count()`
    /// is the whole tree.
    #[must_use]
    pub fn count(&self) -> usize {
        let mut count = 0;
        self.each(&mut |_, _| count += 1);
        count
    }
}

/// `element` with every pixel token multiplied by `factor` — the
/// design's px at the display's own density.
///
/// The renderer's scissor speaks framebuffer pixels, so a 2x window
/// must be drawn at 2x: pads, gaps, sizes, radii, strokes, dashes,
/// pins, glows, text sizes and mark sizes all carry the factor, and
/// the layout runs over the physical viewport. A factor of 1 hands the
/// tree back untouched. The elevation's shadow geometry is the one
/// token that does not scale — it lives inside the emit path — so a 2x
/// shadow is drawn a hair tighter than the design's.
pub(crate) fn scaled(mut element: Element, factor: f32) -> Element {
    if (factor - 1.0).abs() < f32::EPSILON {
        return element;
    }
    for pad in &mut element.pad {
        *pad *= factor;
    }
    element.gap *= factor;
    element.width = element.width.map(|px| px * factor);
    element.height = element.height.map(|px| px * factor);
    element.max_width = element.max_width.map(|px| px * factor);
    element.min_height = element.min_height.map(|px| px * factor);
    element.min_width = element.min_width.map(|px| px * factor);
    element.radius *= factor;
    element.dash = (element.dash.0 * factor, element.dash.1 * factor);
    if let Some((_, width)) = &mut element.stroke {
        *width *= factor;
    }
    if let Some((_, falloff)) = &mut element.glow {
        *falloff *= factor;
    }
    for pin in &mut element.pin {
        *pin = pin.map(|px| px * factor);
    }
    match &mut element.kind {
        Kind::Text { voice, .. } => voice.px *= factor,
        Kind::Grid { px, .. } => *px *= factor,
        Kind::Mark { px, .. } => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let sized = (*px as f32 * factor).round() as u32;
            *px = sized.max(1);
        }
        // A picture's box carries the factor above; the pixels are the
        // picture's own and are drawn into whatever box they are given.
        Kind::Surface | Kind::Picture { .. } => {}
    }
    element.children = element
        .children
        .drain(..)
        .map(|child| scaled(child, factor))
        .collect();
    element
}

#[cfg(test)]
#[path = "element.tests.rs"]
mod tests;
