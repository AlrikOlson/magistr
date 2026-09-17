use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Surface, Widget};
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::layout::{PxRect, Rect};
use magistr_cockpit::theme::ThemeMode;
use magistr_cockpit::widget_id::WidgetId;
use qs_gpu::atlas::ImageKey;
use qs_gpu::color::Srgba;
use qs_gpu::cpu_raster::CpuRasterizer;
use qs_gpu::frame::{DrawList, Instance};
use tiny_skia::Pixmap;

use crate::access::AccessTree;
use crate::chart::Chart;
use crate::comp_birth::BirthComposition;
use crate::comp_feed::FeedComposition;
use crate::comp_galaxy::GalaxyComposition;
use crate::comp_root::RootComposition;
use crate::comp_sky::SkyComposition;
use crate::comp_trace::TraceComposition;
use crate::element::{Element, scaled};
use crate::element_emit::emit;
use crate::element_layout::ElementLayout;
use crate::element_state::ElementState;
use crate::error::Gui2Error;
use crate::galaxy::Galaxy;
use crate::galaxy_layout::GalaxyLayout;
use crate::image_quad::ImageQuad;
use crate::lexicon::Lexicon;
use crate::marks::Marks;
use crate::medium::Medium;
use crate::metrics::CellMetrics;
use crate::radar::Radar;
use crate::sky::Sky;
use crate::text::TextEngine;
use crate::trace_graph::TraceGraph;
use crate::type_scale::TypeScale;

/// A rendered native frame and its hit registry.
///
/// [`Frame::build`] maps the shared model through [`RootComposition`], lays out the
/// pixel element tree with [`ElementLayout`], and draws its contents. It adds the
/// constellation and streamed images in the reserved regions. Widget hit boxes come
/// from this same layout, keeping input coordinates aligned with the rendered controls.
///
/// [`Frame::rasterize`] renders the draw list on the CPU for headless checks and golden
/// comparisons.
pub struct Frame {
    text: TextEngine,
    list: DrawList,
    /// The last built (density-scaled) tree, kept so input can fold
    /// hover/press/focus over it and access can be derived from it.
    tree: Option<Element>,
    /// Where layout put the last tree.
    layout: Option<ElementLayout>,
    /// Per element (tree order): the cockpit widget behind its id, if
    /// any — computed once per frame and read by both the hit
    /// registration and the accessibility tree.
    targets: Vec<Option<WidgetId>>,
    /// Hover, press and focus — folded from the cockpit's own `Msg`.
    state: ElementState,
    hits: HitRegistry,
    metrics: CellMetrics,
    generation: u64,
    cpu: Option<CpuRasterizer>,
}

impl Frame {
    /// The constellation's ink under a bad day — the volume's
    /// `opacity:.5` on view 14's field.
    const BAD_DAY_FADE: f32 = 0.5;

    /// The most passes a headless render takes before it keeps the
    /// frame.
    ///
    /// A pass can be unfinished for two reasons: the framework admits
    /// at most
    /// [`IMAGE_UPLOADS_PER_FRAME`](qs_gpu::atlas::IMAGE_UPLOADS_PER_FRAME)
    /// pictures a frame — two — so a picture can arrive a frame late;
    /// and the sky field's rect *report* changes what the next pass's
    /// constellation reads (the two-pass loop the terminal runtime runs
    /// as follow-up frames). [`Frame::still`] therefore loops until a
    /// pass reported no rect change and admitted nothing new, and this
    /// cap only bounds a frame that never settles, which would be a bug
    /// and not a bigger cockpit.
    const CONVERGE: u32 = 8;

    /// A frame builder over the embedded faces at the default size.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Metrics`] when the embedded mono face yields no
    /// usable grid — a broken build rather than a runtime condition,
    /// reported rather than panicked because this is a library.
    pub fn new() -> Result<Self, Gui2Error> {
        Self::at(CellMetrics::DEFAULT_PX)
    }

    /// A frame builder drawing text at `px`.
    ///
    /// # Errors
    ///
    /// See [`Frame::new`].
    pub fn at(px: f32) -> Result<Self, Gui2Error> {
        let mut text = TextEngine::embedded();
        let metrics = text.metrics(px).ok_or(Gui2Error::Metrics(px))?;
        Ok(Self {
            text,
            list: DrawList::default(),
            tree: None,
            layout: None,
            targets: Vec::new(),
            state: ElementState::default(),
            hits: HitRegistry::new(),
            metrics,
            generation: 0,
            cpu: None,
        })
    }

    /// Re-derive the grid for a window whose scale factor is `scale`
    /// device pixels per logical pixel. Answers whether anything moved,
    /// so a caller can skip the redraw a settled scale does not need.
    ///
    /// The scale goes into the *face* and not into the grid:
    /// [`CellMetrics::grid`] counts whole cells in a **physical**
    /// viewport, and drawing the face at `DEFAULT_PX × scale` doubles
    /// the cell with the glyph, so the same physical viewport floors to
    /// the same count and the text is `DEFAULT_PX` logical. The element
    /// tree follows through the same anchor — the composition's logical
    /// tokens are multiplied by [`TypeScale::factor`], which is exactly
    /// this scale. The [`TextEngine`] is deliberately **kept**: its
    /// atlas entries are keyed by `(face, glyph, size, phase)`, so a
    /// new size mints new keys and the old ones go unreferenced.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Metrics`] for a scale no window reports — zero,
    /// negative or not a number.
    pub fn rescale(&mut self, scale: f64) -> Result<bool, Gui2Error> {
        let px = CellMetrics::DEFAULT_PX * scale as f32;
        if !px.is_finite() || px <= 0.0 {
            return Err(Gui2Error::Metrics(px));
        }
        if px == self.metrics.size().to_f32() {
            return Ok(false);
        }
        self.metrics = self.text.metrics(px).ok_or(Gui2Error::Metrics(px))?;
        Ok(true)
    }

    /// The cell grid this frame draws on — what a window's pixel size is
    /// turned into a [`Size`](magistr_cockpit::cockpit::Size) with, and what
    /// a pointer position is turned into a cell with.
    #[must_use]
    pub const fn metrics(&self) -> CellMetrics {
        self.metrics
    }

    /// Build the frame for `model` at `clock`, over a `width` x `height`
    /// physical-pixel viewport.
    ///
    /// Answers whether the sky field's rect *report* changed — the
    /// constellation's placement depends on
    /// [`Model::set_roadmap_graph_rect`], so a `true` asks the caller
    /// for a follow-up frame exactly as the terminal runtime schedules
    /// one.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Elements`] if taffy rejects the tree, which a tree
    /// built from [`Element`]s never does.
    pub fn build(
        &mut self,
        model: &mut Model,
        clock: &FrameClock,
        width: u32,
        height: u32,
    ) -> Result<bool, Gui2Error> {
        // Every frame this crate draws is on a window, so the model is
        // told here — the one site — rather than at each producer.
        model.set_surface(Surface::Native);
        // Report image wrapping geometry through the layout-only API.
        let cell = self.metrics.cell_pixels();
        if model.cell_pixels() != Some(cell) {
            model.report_cell_size(Some(magistr_cockpit::pixel::CellSize::new(cell.0, cell.1)));
        }
        let medium = match model.theme().mode() {
            ThemeMode::Light => Medium::Day,
            ThemeMode::Dark => Medium::Night,
        };
        // The compositions speak the volume's logical pixels; the
        // window draws physical ones. One factor carries the whole
        // cockpit across — the same anchor the cell metrics scale on.
        let factor = TypeScale::new(&self.metrics).factor();
        let cell_logical = (
            self.metrics.width() / factor,
            self.metrics.height() / factor,
        );
        #[allow(clippy::cast_precision_loss)]
        let viewport = (width as f32 / factor, height as f32 / factor);
        let root = RootComposition::el(model, medium, cell_logical, viewport, clock);
        let tree = scaled(root, factor);

        self.text.begin_frame();
        let mut layout = ElementLayout::compute(
            &tree,
            width.max(1) as f32,
            height.max(1) as f32,
            &mut self.text,
        )?;

        // The one seam between the element tree's interaction ids and
        // the cockpit's widget vocabulary, resolved once and read by
        // the hit registration and the accessibility tree alike.
        self.targets.clear();
        tree.each(&mut |_, element| {
            self.targets.push(
                element
                    .id
                    .and_then(|id| RootComposition::target_of(model, id)),
            );
        });
        // The stage clips its own subtree: DFS numbering is contiguous
        // per subtree, so the feed's whole window — overflow included —
        // is the index range the scissored batch and the hit clamp both
        // read. Absent (the Sky, a review, rest), nothing is split.
        let stage = subtree_of(&tree, RootComposition::STAGE);
        let clip = stage
            .clone()
            .and_then(|range| scissor_of(layout.rect(range.start), [width, height]));
        // While a scroll glides, the feed paints *between* rows
        // (stage-scroll-anchor-and-rate): its children are carried by
        // the fraction of a row the window has scrolled past, in the
        // surface's own cell. The stage box itself stays, so the
        // scissor and the hit clamp are the box the capture measures,
        // and the rows slide inside it. Before the hits are registered,
        // so a control rides its row.
        if let Some(range) = &stage {
            let shift = stage_shift(model, self.metrics.height());
            if shift != 0.0 {
                layout.translate(range.start + 1..range.end, shift);
            }
        }

        self.hits.clear();
        for (index, target) in self.targets.iter().enumerate() {
            let Some(widget) = target else { continue };
            let mut rect = layout.rect(index);
            // A control on an overflowed feed row is clipped out of the
            // paint by the stage's scissor; its hit box clamps the same
            // way, or a click on the header band would press it.
            if let Some(range) = &stage
                && range.contains(&index)
            {
                let Some(inside) = intersect(rect, layout.rect(range.start)) else {
                    continue;
                };
                rect = inside;
            }
            let [x, y, w, h] = rect;
            if w > 0.0 && h > 0.0 {
                // The tree index is the stacking level: depth-first
                // order is paint order, so the element painted last —
                // an overlay's verb over the session it dims — answers
                // first, the registry's own `(z, index)` rule.
                let z = u16::try_from(index).unwrap_or(u16::MAX);
                self.hits.register_px(PxRect::new(x, y, w, h), z, *widget);
            }
        }

        self.generation += 1;
        self.list.reset(
            [width.max(1), height.max(1)],
            Lexicon::of(medium).base,
            self.generation,
        );
        let field = if let (Some(range), Some(scissor)) = (&stage, clip) {
            emit(
                &tree,
                &layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                0..range.start,
            );
            self.list.end_batch(None, true);
            emit(
                &tree,
                &layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                range.clone(),
            );
            // The stream's pictures are feed content: their quads land
            // inside the stage's batch, or a tall image would overflow
            // the fold the way the rows used to.
            self.pictures(model, &tree, &layout);
            self.list.end_batch(Some(scissor), true);
            emit(
                &tree,
                &layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                range.end..tree.count(),
            );
            self.sky(model, medium, factor, &tree, &layout)
        } else {
            // No stage on this frame — or one clamped off the viewport
            // entirely, whose feed then paints nothing at all.
            let range = stage.clone().unwrap_or(0..0);
            emit(
                &tree,
                &layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                0..range.start,
            );
            // The constellation is drawn into its reservation *at* the
            // field's place in the tree, so what the tree stacks over
            // the field — the bad day's cards (view 14), an overlay's
            // scrim — covers the stars the way it covers the paper.
            let split = index_of(&tree, SkyComposition::FIELD)
                .map_or(range.end, |field| (field + 1).max(range.end));
            emit(
                &tree,
                &layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                range.end..split,
            );
            let field = self.sky(model, medium, factor, &tree, &layout);
            emit(
                &tree,
                &layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                split..tree.count(),
            );
            field
        };
        self.galaxy(model, medium, factor, &tree, &layout);
        self.trace_graph(model, medium, factor, &tree, &layout);
        self.birth(model, medium, factor, &tree, &layout);
        self.list.end_batch(None, true);
        self.list.finish();

        // The field's rect goes back to the model in cells — the same
        // report the terminal driver makes — because the roadmap's dag
        // placement is computed against it; a changed report asks for
        // the follow-up frame that reads it.
        let mut reported = false;
        // The turn's mono content is cut to the column the window
        // actually gave it: the model counts in columns, the
        // composition measures in pixels, so the frame converts what it
        // laid out and reports it. Logical pixels — the units the
        // composition's own padding token is written in. Only while a
        // stage is placed; the Sky and a review have no turn column to
        // measure, and the last report stands until one is laid out
        // again.
        if let Some(range) = &stage {
            let cols =
                FeedComposition::mono_cols(layout.rect(range.start)[2] / factor, cell_logical.0);
            if model.mono_cols() != Some(cols) {
                model.set_mono_cols(Some(cols));
                reported = true;
            }
        }
        if model.roadmap_graph_rect() != field {
            model.set_roadmap_graph_rect(field);
            reported = true;
        }
        // The rail's mini-DAG was the cell rail's; the composed rail
        // has no pixel block, and a model that kept a stale rect would
        // arrange for a graph nobody draws.
        if model.rail_graph_rect().is_some() {
            model.set_rail_graph_rect(None);
            reported = true;
        }
        self.tree = Some(tree);
        self.layout = Some(layout);
        Ok(reported)
    }

    /// The Sky's constellation, drawn as primitives into the field the
    /// root reserved — stars, rings, halos and edges are instances, so
    /// the camera is applied on the frame and every star answers a
    /// click. Answers the field's cell rect for the model's report.
    fn sky(
        &mut self,
        model: &Model,
        medium: Medium,
        factor: f32,
        tree: &Element,
        layout: &ElementLayout,
    ) -> Option<Rect> {
        let index = index_of(tree, SkyComposition::FIELD)?;
        let [x, y, w, h] = layout.rect(index);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let roadmap = model.roadmap();
        // A bad day holds the constellation still and at half its ink
        // (view 14): the stillest screen in the app.
        let bad_day = model.bad_day().is_some();
        let animate = model.theme().motion().is_full() && !bad_day;
        let stars = Sky::new(
            PxRect::new(x, y, w, h),
            (self.metrics.width(), self.metrics.height()),
            factor,
            model.theme().palette(),
            medium,
        )
        .at(model.now(), animate)
        .panned(roadmap.pan())
        .shipping(roadmap.ship_run(model.now()))
        .faded(if bad_day { Self::BAD_DAY_FADE } else { 1.0 })
        .show(
            &model.sky_nodes(model.now()),
            &roadmap.dag_edge_kinds(),
            &mut self.list,
        );
        // One level above the field itself, so a star beats the field
        // (whose click still pans) and loses to any overlay declared
        // after it.
        let star_z = u16::try_from(index + 1).unwrap_or(u16::MAX);
        for (star, rect) in stars {
            self.hits.register_px(rect, star_z, Widget::sky_star(star));
        }
        let (cw, ch) = (self.metrics.width(), self.metrics.height());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rect = Rect::new(
            (x / cw) as u16,
            (y / ch) as u16,
            (w / cw) as u16,
            (h / ch) as u16,
        );
        (!rect.is_empty()).then_some(rect)
    }

    /// The Galaxy's clusters, drawn as primitives into the field the
    /// root reserved at altitude 0, from the same layout the
    /// composition pinned the labels with; every cluster's box is
    /// registered so a click on it is a dive.
    fn galaxy(
        &mut self,
        model: &Model,
        medium: Medium,
        factor: f32,
        tree: &Element,
        layout: &ElementLayout,
    ) {
        let Some(index) = index_of(tree, GalaxyComposition::FIELD) else {
            return;
        };
        let [x, y, w, h] = layout.rect(index);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let projects = GalaxyComposition::projects(model);
        let placed =
            GalaxyLayout::place(&projects, (w / factor, h / factor), model.galaxy().zoom());
        let animate = model.theme().motion().is_full();
        let clusters = Galaxy::new(PxRect::new(x, y, w, h), factor, Lexicon::of(medium), medium)
            .at(model.now(), animate)
            .selecting(model.galaxy().selected())
            .show(&placed, &mut self.list);
        // One level above the field itself, so a cluster beats the
        // field (whose wheel still zooms) and loses to any overlay
        // declared after it.
        let z = u16::try_from(index + 1).unwrap_or(u16::MAX);
        for (cluster, rect) in clusters {
            self.hits
                .register_px(rect, z, Widget::galaxy_cluster(cluster));
        }
    }

    /// The Trace view's branch graph, drawn as primitives into the field
    /// the rail reserved, from the same lane the composition derives;
    /// every dot is registered as its step's trace row, so a click on
    /// the graph is the click on the card.
    fn trace_graph(
        &mut self,
        model: &Model,
        medium: Medium,
        factor: f32,
        tree: &Element,
        layout: &ElementLayout,
    ) {
        let Some(index) = index_of(tree, TraceComposition::GRAPH) else {
            return;
        };
        let [x, y, w, h] = layout.rect(index);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let dots = TraceGraph::new(PxRect::new(x, y, w, h), factor, medium)
            .show(&TraceComposition::lane(model), &mut self.list);
        // One level above the field, like a star over the sky.
        let z = u16::try_from(index + 1).unwrap_or(u16::MAX);
        for (step, rect) in dots {
            self.hits.register_px(rect, z, Widget::trace_row(step));
        }
    }

    /// Birth's field drawings: the survey's sweep and points into the field the survey
    /// reserved, the charting's constellation into the charting's — each a pure
    /// function of the tick the model is read at. Nothing here is hittable: the sweep
    /// is a picture and the proposal's rows are the list's.
    fn birth(
        &mut self,
        model: &Model,
        medium: Medium,
        factor: f32,
        tree: &Element,
        layout: &ElementLayout,
    ) {
        let Some(index) = index_of(tree, BirthComposition::FIELD) else {
            return;
        };
        let [x, y, w, h] = layout.rect(index);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let birth = model.birth();
        let animate = model.theme().motion().is_full();
        let lex = Lexicon::of(medium);
        let field = PxRect::new(x, y, w, h);
        match birth.moment() {
            magistr_cockpit::cockpit::BirthMoment::Survey => {
                let phase = if animate {
                    birth.radar_phase(model.now())
                } else {
                    0.0
                };
                let points = birth.survey().map(|r| r.points.as_slice()).unwrap_or(&[]);
                Radar::new(field, factor, lex, medium)
                    .at(phase)
                    .show(points, &mut self.list);
            }
            magistr_cockpit::cockpit::BirthMoment::Charting => {
                let Some(proposal) = birth.proposal() else {
                    return;
                };
                let alphas: Vec<f32> = (0..proposal.chunks.len())
                    .map(|i| birth.born_alpha(i, model.now(), animate))
                    .collect();
                Chart::new(field, factor, lex, medium).show(proposal, &alphas, &mut self.list);
            }
            _ => {}
        }
    }

    /// The stream's pictures at their own pixels, into the footprints
    /// the feed reserved. A picture is admitted **once**, under its own
    /// digest, and drawn at 1:1 when the footprint holds it — fit to
    /// the box with its aspect kept when the footprint had to shrink.
    fn pictures(&mut self, model: &Model, tree: &Element, layout: &ElementLayout) {
        let page = self.text.colour_size();
        let mut slots: Vec<(usize, usize)> = Vec::new();
        tree.each(&mut |index, element| {
            if let Some(at) = element.id.and_then(FeedComposition::image_row) {
                slots.push((index, at));
            }
        });
        for (index, at) in slots {
            let Some((image, 0, _)) = model.turn().image_at_row(at) else {
                continue;
            };
            let [x, y, w, h] = layout.rect(index);
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            let Some(entry) = self.text.image(ImageKey(image.digest()), |_| {
                Some(ImageQuad::admission(image, page))
            }) else {
                continue;
            };
            let footprint = PxRect::new(x, y, w, h);
            let size = (u32::from(image.width()), u32::from(image.height()));
            let Some(quad) = ImageQuad::place(footprint, footprint, size, entry.uv) else {
                continue;
            };
            self.list.instances.push(Instance::image(
                quad.rect.x,
                quad.rect.y,
                quad.rect.width,
                quad.rect.height,
                quad.uv,
                Srgba::new(1.0, 1.0, 1.0, 1.0),
            ));
        }
    }

    /// Fold one cockpit message over the last built frame's tree —
    /// hover, press and focus. Answers whether anything changed, which
    /// is the caller's cue to paint.
    pub fn fold_state(&mut self, msg: &Msg) -> bool {
        let (Some(tree), Some(layout)) = (&self.tree, &self.layout) else {
            return false;
        };
        self.state.fold(msg, tree, layout)
    }

    /// The cockpit widget behind the element holding keyboard focus —
    /// what ↵ presses. `None` with no focus, and for a focused element
    /// that is visual state only.
    #[must_use]
    pub fn focus_target(&self) -> Option<WidgetId> {
        let focus = self.state.focus()?;
        let tree = self.tree.as_ref()?;
        let mut target = None;
        tree.each(&mut |index, element| {
            if element.id == Some(focus) && target.is_none() {
                target = self.targets.get(index).copied().flatten();
            }
        });
        target
    }

    /// Whether the arrow keys are the frame's to spend — focus is on a
    /// walkable element (a feed card's head), so ↑/↓ move it rather
    /// than reaching the cockpit.
    #[must_use]
    pub fn focus_walks(&self) -> bool {
        self.tree
            .as_ref()
            .is_some_and(|tree| self.state.walks(tree))
    }

    /// The draw list the last [`Frame::build`] produced.
    #[must_use]
    pub const fn list(&self) -> &DrawList {
        &self.list
    }

    /// The hit registry of the last frame — the placed element boxes
    /// under the cockpit's own widget ids, which is why the native GUI
    /// has no second hit path.
    #[must_use]
    pub const fn hits(&self) -> &HitRegistry {
        &self.hits
    }

    /// The last built element tree and its layout, for callers that
    /// read the placed boxes — `None` before the first build.
    #[must_use]
    pub fn placed(&self) -> Option<(&Element, &ElementLayout)> {
        match (&self.tree, &self.layout) {
            (Some(tree), Some(layout)) => Some((tree, layout)),
            _ => None,
        }
    }

    /// Where the last built frame placed the element wearing `id` —
    /// `None` before the first build and for an id no composition
    /// minted this frame.
    #[must_use]
    pub fn rect_of(&self, id: crate::element_id::ElementId) -> Option<[f32; 4]> {
        let (tree, layout) = self.placed()?;
        let mut found = None;
        tree.each(&mut |index, element| {
            if element.id == Some(id) && found.is_none() {
                found = Some(layout.rect(index));
            }
        });
        found
    }

    /// Every text run of the last built frame with its placed box, in
    /// tree (which is paint) order — what the frame *says*, which is
    /// the inventory the harness's wire-less assertions and the
    /// accessible names read.
    #[must_use]
    pub fn says(&self) -> Vec<(String, [f32; 4])> {
        let (Some(tree), Some(layout)) = (&self.tree, &self.layout) else {
            return Vec::new();
        };
        let mut said = Vec::new();
        tree.each(&mut |index, element| {
            if let crate::element::Kind::Text { content, .. } = &element.kind {
                said.push((content.clone(), layout.rect(index)));
            }
        });
        said
    }

    /// The accessibility tree for the frame that was last built, at a
    /// window scale factor of `scale` — derived from this frame's own
    /// placed elements, and therefore unable to describe a screen the
    /// emitter did not draw. Empty (the window node alone) before the
    /// first build.
    #[must_use]
    pub fn access(&self, scale: f64) -> AccessTree {
        match (&self.tree, &self.layout) {
            (Some(tree), Some(layout)) => AccessTree::of(tree, layout, &self.targets, scale),
            _ => AccessTree::default(),
        }
    }

    /// The engine that shaped this frame's text and owns its atlas.
    pub fn text(&mut self) -> &mut TextEngine {
        &mut self.text
    }

    /// Build `model`'s frame repeatedly until the atlas has admitted
    /// everything it draws and the field report has settled, then
    /// rasterize it on the CPU tier.
    ///
    /// The headless path, and what a golden is a picture of. It exists
    /// as its own verb rather than as a flag on `build` because
    /// convergence is a property of the *render*, not of the frame: a
    /// window converges over its own next frames and must not spend
    /// three of them at once.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Elements`] from the layout pass, or
    /// [`Gui2Error::Raster`] when the CPU tier refuses the viewport.
    pub fn still(
        &mut self,
        model: &mut Model,
        clock: &FrameClock,
        width: u32,
        height: u32,
    ) -> Result<Pixmap, Gui2Error> {
        for _ in 0..Self::CONVERGE {
            let reported = self.build(model, clock, width, height)?;
            let uploaded = self.apply_uploads(width, height)?;
            if !reported && !uploaded {
                break;
            }
        }
        self.rasterize(width, height)
    }

    /// The Almagest set's specimen sheet, rasterized on the CPU tier.
    ///
    /// The counterpart of [`Frame::still`] for a surface that has no
    /// `Model`: [`Marks::specimen`] emits its quads through the same
    /// atlas page, the same admission bound and the same rasterizer a
    /// mark placed on a real cockpit surface would take, so the golden
    /// is a picture of the painter rather than of a second drawing
    /// beside it.
    ///
    /// It needs its own convergence bound and cannot reuse
    /// [`Frame::CONVERGE`]: the framework admits two pictures a frame
    /// and the sheet holds [`Marks::SPECIMEN`] of them, so a whole
    /// sheet is tens of passes rather than a handful. The loop ends on
    /// the pass that drew every mark; only that pass's list is
    /// rasterized.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Raster`] when the CPU tier refuses the viewport, and
    /// [`Gui2Error::Elements`] never — there is no layout pass here.
    pub fn specimen(&mut self, marks: &Marks) -> Result<Pixmap, Gui2Error> {
        let (width, height) = Marks::sheet();
        // Two admissions a frame, plus a pass to spare for the one that
        // draws them all.
        let passes = Marks::SPECIMEN + 1;
        for _ in 0..passes {
            self.text.begin_frame();
            self.generation += 1;
            self.list
                .reset([width, height], marks.ground(), self.generation);
            let drawn = marks.specimen(&mut self.text, &mut self.list);
            self.list.end_batch(None, true);
            self.list.finish();
            self.apply_uploads(width, height)?;
            if drawn == Marks::SPECIMEN {
                break;
            }
        }
        self.rasterize(width, height)
    }

    /// Copy this frame's newly admitted glyphs and pictures into the CPU
    /// tier's mirrors. Answers whether there was anything to copy — a
    /// frame that admitted nothing is a settled one.
    fn apply_uploads(&mut self, width: u32, height: u32) -> Result<bool, Gui2Error> {
        let glyphs = self.text.take_uploads();
        let images = self.text.take_image_uploads();
        let uploaded = !glyphs.is_empty() || !images.is_empty();
        let cpu = self.cpu_tier(width, height)?;
        cpu.upload_glyphs(&glyphs);
        cpu.upload_images(&images);
        Ok(uploaded)
    }

    /// The CPU rasterizer, sized to the viewport and to *this* engine's
    /// two atlas pages — a mirror sized from the wrong page reads the
    /// right texture at the wrong coordinates.
    fn cpu_tier(&mut self, width: u32, height: u32) -> Result<&mut CpuRasterizer, Gui2Error> {
        if width == 0 || height == 0 {
            return Err(Gui2Error::Raster(width, height));
        }
        let (atlas, colour) = (self.text.atlas_size(), self.text.colour_size());
        let fits = self.cpu.as_mut().is_some_and(|cpu| {
            (cpu.width() == width && cpu.height() == height) || cpu.resize(width, height)
        });
        if !fits {
            // A fresh mirror is blank, so everything the atlas has ever
            // admitted has to be replayed into it — and the atlas hands
            // an upload out exactly once. Taking the pages this way
            // round would lose them, so a mirror is only ever built
            // before the frame that fills it.
            self.cpu = Some(
                CpuRasterizer::with_colour_page(width, height, atlas, colour)
                    .ok_or(Gui2Error::Raster(width, height))?,
            );
        }
        self.cpu.as_mut().ok_or(Gui2Error::Raster(width, height))
    }

    /// The last built list, rasterized on the CPU tier — in displayable sRGB.
    ///
    /// The tier's working buffer is **linear** premultiplied and its own contract says
    /// to call [`CpuRasterizer::to_srgb_rgba8`] before showing it; returning the
    /// working buffer raw is what made every native still a too-dark picture of the
    /// theme (a Mocha base of `#1e1e2e` came out `#030307`), which the mock parity gate
    /// measured as the whole canvas differing. The straight sRGB bytes are written into
    /// an opaque pixmap directly: a frame's canvas is opaque, so straight and
    /// premultiplied agree byte for byte, and
    /// `the_still_is_the_viewport_and_every_pixel_is_opaque` pins the assumption.
    fn rasterize(&mut self, width: u32, height: u32) -> Result<Pixmap, Gui2Error> {
        let list = std::mem::take(&mut self.list);
        let outcome = self.cpu_tier(width, height).and_then(|cpu| {
            cpu.render(&list);
            let bytes = cpu.to_srgb_rgba8();
            let mut pixmap = Pixmap::new(width, height).ok_or(Gui2Error::Raster(width, height))?;
            pixmap.data_mut().copy_from_slice(&bytes);
            Ok(pixmap)
        });
        self.list = list;
        outcome
    }
}

/// The tree index of the element wearing `id`, if any.
fn index_of(tree: &Element, id: crate::element_id::ElementId) -> Option<usize> {
    let mut found = None;
    tree.each(&mut |index, element| {
        if element.id == Some(id) && found.is_none() {
            found = Some(index);
        }
    });
    found
}

/// The DFS index range of the subtree rooted at the element wearing
/// `id` — contiguous by [`Element::each`]'s own numbering, which is
/// what lets the frame emit it as one batch.
fn subtree_of(tree: &Element, id: crate::element_id::ElementId) -> Option<std::ops::Range<usize>> {
    let mut found = None;
    tree.each(&mut |index, element| {
        if element.id == Some(id) && found.is_none() {
            found = Some(index..index + element.count());
        }
    });
    found
}

/// The pixels the stage's rows are carried by while a scroll glides
/// between rows: the fraction of a row scrolled past
/// ([`Model::stage_scroll_fraction`]) in the cell's own `row_px`. A
/// pinned window anchors its rows at the top, so the first row is on
/// its way *out* and the rows move up by the fraction; a feed at its
/// tail anchors at the bottom, where the extra row the fraction let on
/// is on its way *in*, so the rows move down by the rest of it. Zero
/// on a settled frame, so a still is the still it always was.
fn stage_shift(model: &Model, row_px: f32) -> f32 {
    let fraction = model.stage_scroll_fraction();
    if fraction <= 0.0 {
        return 0.0;
    }
    if RootComposition::feed_at_its_tail(model) {
        (1.0 - fraction) * row_px
    } else {
        -fraction * row_px
    }
}

/// A layout box as a wgpu-legal scissor: clamped inside the viewport —
/// the API refuses a rect outside the render target — and `None` when
/// nothing of it is on screen, because a zero-sized scissor is refused
/// too.
fn scissor_of(rect: [f32; 4], viewport: [u32; 2]) -> Option<[u32; 4]> {
    let [x, y, w, h] = rect;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let clamp = |v: f32, limit: u32| (v.max(0.0) as u32).min(limit);
    let x0 = clamp(x.floor(), viewport[0]);
    let y0 = clamp(y.floor(), viewport[1]);
    let x1 = clamp((x + w).ceil(), viewport[0]);
    let y1 = clamp((y + h).ceil(), viewport[1]);
    (x1 > x0 && y1 > y0).then(|| [x0, y0, x1 - x0, y1 - y0])
}

/// The overlap of two layout boxes, `None` when they only touch or
/// miss — the hit clamp's arithmetic.
fn intersect(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

#[cfg(test)]
#[path = "frame.tests.rs"]
mod tests;
