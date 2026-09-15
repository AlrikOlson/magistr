//! The frame's pixel surfaces: what the cockpit draws with pixels
//! rather than with cells, and the caches that keep an unchanged one
//! from costing a byte.
//!
//! This is deliberately *not* part of [`Driver`](super::driver::Driver).
//! The runtime drives it once a frame, and so does
//! [`screenshot::frame`](crate::screenshot::frame) when its terminal
//! profile has a graphics protocol — one
//! route, so a golden proves the pixels the runtime would really place
//! rather than a lookalike built beside them. Everything it needs is in
//! its arguments: the model (read, plus the two graph-rect reports it
//! owes back), where layout put things, and the screen to show them on.

use crate::render::Screen;
use magistr_cockpit::cockpit::view::ROADMAP_LEGEND_COLS;
use magistr_cockpit::cockpit::view::{activity_series, rail_gauge_cols, spark_sweep};
use magistr_cockpit::cockpit::{DagNode, DagPath, Model, Motion, Widget};
use magistr_cockpit::layout::{Rect, Resolved};
use magistr_cockpit::pixel::{
    BranchGraph, Brand, CellSize, Gauge, Image, PixelSurface, RoadmapDag, Sparkline,
};
use magistr_cockpit::render::Color;
use magistr_cockpit::theme::Pen;
use magistr_cockpit::turn::{Accent, RowKind};
use magistr_cockpit::widget_id::WidgetId;

/// What the cached branch graph was painted from: the steps' lanes, the
/// fork links, the rect it covers, the cell size it was scaled by and
/// the frame of the sweep drawing it in (`None` once it is settled).
type GraphKey = (
    Vec<(u32, u8)>,
    Vec<(usize, usize)>,
    Rect,
    CellSize,
    Option<u32>,
);

/// What the cached tech-tree was painted from: the placed nodes (the
/// camera's glide already applied), the dependency edges, the rect it
/// covers, the cell size and the phase the active chunk's glow was
/// breathing at.
type DagKey = (
    Vec<DagNode>,
    Vec<(usize, usize)>,
    Vec<DagPath>,
    Rect,
    CellSize,
    f32,
);

/// What the cached header sparkline was painted from: the series, its
/// rect, the cell size and the frame of the sweep drawing it in
/// (`None` once it is settled).
type SparkKey = (Vec<u32>, Rect, CellSize, Option<u32>);

/// What a cached stream image was painted from: the turn row its
/// placement starts at, the rows of it on screen, the rect it covers and
/// the cell size — the image itself never changes once decoded, so its
/// starting row identifies it.
type ImageKey = (usize, u16, u16, Rect, CellSize);

/// What a cached one-shot chrome surface was painted from: a fraction
/// (the gauge's fill; the brand's is always zero), its rect, the cell
/// size and the pen colour it wears.
type ChromeKey = (u32, Rect, CellSize, (u8, u8, u8));

/// One run of an image's rows on the stage: the turn row its placement
/// starts at, the visible index the run starts at, which rows of the
/// placement they are, and the cells they cover.
#[derive(Clone, Copy, Debug)]
struct Band {
    start: usize,
    index: usize,
    first: u16,
    count: u16,
    rect: Rect,
}

/// Every pixel surface the cockpit shows, and what each was painted
/// from.
///
/// [`Surfaces::show`] is the whole entry point: run it after the cell
/// buffer is painted and before [`Screen::present`], and the frame
/// carries the brand mark, the activity sparkline, the rail gauge, the
/// stream's images, the Trace tab's branch graph and both tech-trees.
#[derive(Debug, Default)]
pub(crate) struct Surfaces {
    /// The Trace tab's branch graph as a pixel surface, kept between
    /// frames so an unchanged graph costs zero bytes.
    graph: Option<PixelSurface>,
    /// What the cached graph was painted from.
    graph_key: Option<GraphKey>,
    /// The Roadmap tab's tech-tree as a pixel surface, likewise cached.
    dag: Option<PixelSurface>,
    /// What the cached tech-tree was painted from.
    dag_key: Option<DagKey>,
    /// The rail's mini-DAG as a pixel surface, likewise cached.
    rail: Option<PixelSurface>,
    /// What the cached mini-DAG was painted from.
    rail_key: Option<DagKey>,
    /// The header's brand mark, an alpha surface over its one cell.
    brand: Option<PixelSurface>,
    /// What the cached brand mark was painted from.
    brand_key: Option<ChromeKey>,
    /// The header's activity sparkline.
    spark: Option<PixelSurface>,
    /// What the cached sparkline was painted from.
    spark_key: Option<SparkKey>,
    /// The rail's roadmap gauge.
    gauge: Option<PixelSurface>,
    /// What the cached gauge was painted from.
    gauge_key: Option<ChromeKey>,
    /// The stream's images, one surface per placement visible on the
    /// stage, keyed by the widget id of its first visible row.
    images: Vec<(WidgetId, PixelSurface, ImageKey)>,
}

impl Surfaces {
    /// Every surface of this frame, in the order the runtime has always
    /// shown them. Says whether a graph rect *report* changed — the two
    /// tech-trees feed their placed rect back to the model, and the
    /// caller schedules a follow-up frame because the tab's layout
    /// depends on it.
    pub(crate) fn show(
        &mut self,
        model: &mut Model,
        resolved: &Resolved,
        screen: &mut Screen,
    ) -> bool {
        self.show_trace_graph(model, resolved, screen);
        self.show_chrome(model, resolved, screen);
        self.show_stage_images(model, resolved, screen);
        self.show_roadmap_graph(model, resolved, screen)
            | self.show_rail_graph(model, resolved, screen)
    }

    /// The chrome's three small pixel surfaces — the header's brand mark
    /// and activity sparkline, the rail's roadmap gauge — over the cells
    /// the view left blank for them ([`Model::has_pixels`]). Each caches
    /// on what it was painted from, like the graphs, so a frame that
    /// changed none of them costs zero graphics bytes; without graphics
    /// nothing is shown and the view's cell drawings stand instead.
    fn show_chrome(&mut self, model: &Model, resolved: &Resolved, screen: &mut Screen) {
        if !screen.has_graphics() {
            return;
        }
        let cell = model.cell_size();
        let pen = Pen::for_theme(&model.theme());
        // The painters take plain RGB; a pen colour the palette left at
        // the terminal's default falls back to the theme's own ink.
        let rgb = |color: Color, fallback: (u8, u8, u8)| match color {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => fallback,
        };
        let ground = rgb(pen.base.bg, (26, 27, 38));

        // Give the mark a two-cell drawing surface so its route remains
        // legible at terminal cell resolution; the product name follows
        // the reserved two-cell brand slot.
        if let Some(rect) =
            placed(resolved, Widget::HeaderBrand).map(|slot| Rect::new(slot.x, slot.y, 2, 1))
        {
            let (r, g, b) = rgb(pen.header.fg, (205, 214, 244));
            let key = (0, rect, cell, (r, g, b));
            if self.brand_key != Some(key) {
                let surface = self
                    .brand
                    .get_or_insert_with(|| PixelSurface::with_alpha(rect, cell));
                surface.set_rect(rect);
                surface.set_cell_size(cell);
                surface.paint(&Brand::new(r, g, b));
                self.brand_key = Some(key);
            }
            if let Some(surface) = &mut self.brand {
                screen.surface(Widget::HeaderBrand.id(), surface);
            }
        }

        if let Some(rect) = placed(resolved, Widget::HeaderSpark) {
            let series: Vec<u32> = activity_series(model)
                .into_iter()
                .map(|v| v as u32)
                .collect();
            let header = rgb(pen.footer.bg, ground);
            // Keyed by the sweep's thousandth like the trace graph's, so
            // the monitor repaints once per step while it draws itself
            // in and — once the sweep is gone — stops for good.
            let sweep = spark_sweep(model);
            let key = (
                series.clone(),
                rect,
                cell,
                sweep.map(|t| (t * 1000.0) as u32),
            );
            if self.spark_key.as_ref() != Some(&key) {
                let (r, g, b) = rgb(pen.note.fg, (147, 153, 178));
                let values: Vec<f32> = series.iter().map(|&n| n as f32).collect();
                let line = Sparkline::new(values)
                    .line(r, g, b)
                    .background(header.0, header.1, header.2)
                    .sweep(sweep);
                let surface = self
                    .spark
                    .get_or_insert_with(|| PixelSurface::new(rect, cell));
                surface.set_rect(rect);
                surface.set_cell_size(cell);
                surface.paint(&line);
                self.spark_key = Some(key);
            }
            if let Some(surface) = &mut self.spark {
                screen.surface(Widget::HeaderSpark.id(), surface);
            }
        }

        // The gauge's surface covers the row's bar, not its label.
        if let Some(rect) = placed(resolved, Widget::RailGauge)
            .map(|row| Rect::new(row.x, row.y, rail_gauge_cols(model).min(row.width), 1))
            .filter(|bar| !bar.is_empty())
        {
            let fill = model.motion().rail_fill(model.now());
            let rail_ground = rgb(pen.stage.bg, ground);
            let (r, g, b) = rgb(pen.rail.fg, (205, 214, 244));
            // The fill is keyed by its thousandth: finer than any bar is
            // wide, coarse enough that a settled gauge stops repainting.
            let key = ((fill * 1000.0) as u32, rect, cell, (r, g, b));
            if self.gauge_key != Some(key) {
                let gauge = Gauge::new(fill).line(r, g, b).background(
                    rail_ground.0,
                    rail_ground.1,
                    rail_ground.2,
                );
                let surface = self
                    .gauge
                    .get_or_insert_with(|| PixelSurface::new(rect, cell));
                surface.set_rect(rect);
                surface.set_cell_size(cell);
                surface.paint(&gauge);
                self.gauge_key = Some(key);
            }
            if let Some(surface) = &mut self.gauge {
                screen.surface(Widget::RailGauge.id(), surface);
            }
        }
    }

    /// The stream's images as pixel surfaces, over the cells the view
    /// left blank for them.
    ///
    /// A placement is a run of consecutive [`RowKind::Image`] rows in
    /// the stage's window; its surface covers the rows on screen, and
    /// [`Image::rows`] draws the slice of the image those rows hold, so
    /// an image scrolling past the top of the stage moves under the
    /// window rather than squashing into it. Each surface caches on what
    /// it was painted from, so a still stage costs zero graphics bytes;
    /// a placement that scrolled out of the window is dropped and the
    /// compositor deletes its placement.
    fn show_stage_images(&mut self, model: &Model, resolved: &Resolved, screen: &mut Screen) {
        if !screen.has_graphics() {
            self.images.clear();
            return;
        }
        let cell = model.cell_size();
        let pen = Pen::for_theme(&model.theme());
        let ground = match pen.row(RowKind::Image).bg {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (26, 27, 38),
        };
        let mut shown: Vec<(WidgetId, PixelSurface, ImageKey)> = Vec::new();
        for band in Self::image_bands(model, resolved) {
            let key = (band.start, band.first, band.count, band.rect, cell);
            let id = Widget::stage_row(band.index);
            let cached = self
                .images
                .iter()
                .position(|(known, _, known_key)| *known == id && *known_key == key);
            let mut surface = match cached {
                Some(at) => self.images.remove(at).1,
                None => {
                    let Some((image, _, (_, total))) = model
                        .turn()
                        .image_at_row(band.start + usize::from(band.first))
                    else {
                        continue;
                    };
                    let painter = Image::new(image.rgba(), image.width(), image.height())
                        .rows(band.first, band.count, total)
                        .background(ground.0, ground.1, ground.2);
                    let mut surface = PixelSurface::new(band.rect, cell);
                    surface.paint(&painter);
                    surface
                }
            };
            surface.set_rect(band.rect);
            surface.set_cell_size(cell);
            screen.surface(id, &mut surface);
            shown.push((id, surface, key));
        }
        self.images = shown;
    }

    /// The stage's image placements this frame.
    ///
    /// Each band's rect is its run of stage rows minus the accent
    /// gutter, cut to the placement's own width, so the surface sits
    /// exactly where the view left blanks.
    fn image_bands(model: &Model, resolved: &Resolved) -> Vec<Band> {
        let top = model.top_row();
        let mut bands: Vec<Band> = Vec::new();
        for (index, row) in model.visible_rows().iter().enumerate() {
            if row.kind != RowKind::Image {
                continue;
            }
            let Some((_, offset, (cols, _))) = model.turn().image_at_row(top + index) else {
                continue;
            };
            let Some(rect) = placed_id(resolved, Widget::stage_row(index))
                .map(|row| {
                    Rect::new(
                        row.x + Accent::GUTTER_COLS,
                        row.y,
                        cols.min(row.width.saturating_sub(Accent::GUTTER_COLS)),
                        1,
                    )
                })
                .filter(|rect| !rect.is_empty())
            else {
                continue;
            };
            let start = (top + index) - usize::from(offset);
            match bands.last_mut() {
                // The next row of the same placement, directly below:
                // the band grows instead of starting again.
                Some(band)
                    if band.start == start
                        && band.first + band.count == offset
                        && band.rect.x == rect.x
                        && band.rect.width == rect.width =>
                {
                    band.count += 1;
                    band.rect.height += 1;
                }
                _ => bands.push(Band {
                    start,
                    index,
                    first: offset,
                    count: 1,
                    rect,
                }),
            }
        }
        bands
    }

    /// The Trace tab's branch graph in pixels, over the cells of its
    /// block — only when the terminal composites graphics; the cell view
    /// keeps the indented list otherwise. The cell size is the model's
    /// [`cell_size`](Model::cell_size): the 10×20 guess until the
    /// terminal answers, and a late `CSI 16 t` reply re-sizes the cached
    /// surface through [`PixelSurface::set_cell_size`]. The surface
    /// repaints only when the graph, its rect or the cell size changed,
    /// so an unchanged frame costs zero graphics bytes; a frame without
    /// the block (another tab, no branches) shows no surface and the
    /// compositor deletes the placement.
    ///
    /// While the trace has just grown, the graph draws itself in from
    /// the left over [`Motion::SWEEP_STEPS`](magistr_cockpit::cockpit::Motion::SWEEP_STEPS)
    /// — one repaint per animation step, and none once the sweep is
    /// over, so a settled Trace tab is the frame it always was.
    fn show_trace_graph(&mut self, model: &Model, resolved: &Resolved, screen: &mut Screen) {
        if !screen.has_graphics() {
            return;
        }
        let rect = resolved
            .placed()
            .iter()
            .find_map(|placed| (placed.id == Some(Widget::TraceGraph.id())).then_some(placed.rect));
        let Some(rect) = rect else {
            return;
        };
        if rect.is_empty() {
            return;
        }
        let cell = model.cell_size();
        let trace = model.trace();
        // The sweep is keyed by its thousandth, like the gauge's fill:
        // finer than any graph is wide, and coarse enough that a settled
        // graph — sweep gone, `None` — stops repainting for good.
        let sweep = model.motion().trace_sweep(trace.steps().len(), model.now());
        let key = (
            trace.lanes(),
            trace.links(),
            rect,
            cell,
            sweep.map(|t| (t * 1000.0) as u32),
        );
        if self.graph_key.as_ref() != Some(&key) {
            let lanes: Vec<u8> = key.0.iter().map(|&(_, lane)| lane).collect();
            let mut graph = BranchGraph::new(lanes, key.1.clone()).sweep(sweep);
            if let Some(Color::Rgb(r, g, b)) = model.background() {
                graph = graph.background(r, g, b);
            }
            let surface = self
                .graph
                .get_or_insert_with(|| PixelSurface::new(rect, cell));
            surface.set_rect(rect);
            surface.set_cell_size(cell);
            surface.paint(&graph);
            self.graph_key = Some(key);
        }
        if let Some(surface) = &mut self.graph {
            screen.surface(Widget::TraceGraph.id(), surface);
        }
    }

    /// The Roadmap tab's tech-tree in pixels, beside the cells of its
    /// block — the same caching discipline as
    /// [`Surfaces::show_trace_graph`]. The surface covers only the block
    /// right of the legend gutter, so the workstream names stay text and
    /// pixels never sit over glyphs; that sub-rect is what the model
    /// gets ([`Model::set_roadmap_graph_rect`], `None` while another tab
    /// shows or graphics are off), so mouse events and the fit share the
    /// painter's geometry. Says whether the report changed — the caller
    /// schedules a follow-up frame, since the tab's layout depends on it.
    fn show_roadmap_graph(
        &mut self,
        model: &mut Model,
        resolved: &Resolved,
        screen: &mut Screen,
    ) -> bool {
        let rect = resolved
            .placed()
            .iter()
            .find_map(|placed| {
                (placed.id == Some(Widget::RoadmapGraph.id())).then_some(placed.rect)
            })
            .and_then(|block| {
                (block.width > ROADMAP_LEGEND_COLS + 8 && block.height > 0).then(|| {
                    Rect::new(
                        block.x + ROADMAP_LEGEND_COLS,
                        block.y,
                        block.width - ROADMAP_LEGEND_COLS,
                        block.height,
                    )
                })
            });
        let report = rect.filter(|_| screen.has_graphics());
        let report_changed = model.roadmap_graph_rect() != report;
        if report_changed {
            model.set_roadmap_graph_rect(report);
        }
        if !screen.has_graphics() {
            return report_changed;
        }
        let Some(rect) = rect else {
            return report_changed;
        };
        let cell = model.cell_size();
        let roadmap = model.roadmap();
        let nodes = roadmap.dag_nodes_at(model.now());
        let paths = roadmap.dag_paths_at(model.now());
        let key = (
            nodes,
            roadmap.dag_edges(),
            paths,
            rect,
            cell,
            Self::glow_phase(model),
        );
        if self.dag_key.as_ref() != Some(&key) {
            let mut dag = RoadmapDag::new(key.0.clone(), key.1.clone(), (cell.width, cell.height))
                .routes(key.2.clone())
                .glow(key.5);
            if let Some(Color::Rgb(r, g, b)) = model.background() {
                dag = dag.background(r, g, b);
            }
            let surface = self
                .dag
                .get_or_insert_with(|| PixelSurface::new(rect, cell));
            surface.set_rect(rect);
            surface.set_cell_size(cell);
            surface.paint(&dag);
            self.dag_key = Some(key);
        }
        if let Some(surface) = &mut self.dag {
            screen.surface(Widget::RoadmapGraph.id(), surface);
        }
        report_changed
    }

    /// Where the active chunk's glow is in its breath, for the two
    /// tech-tree surfaces' cache keys: the phase off
    /// [`Model::ticks`], or a flat zero under reduced motion — the same
    /// gate `view`'s `in_motion` applies to every cell surface.
    ///
    /// A key that carries the phase is what re-rasterizes the tree on
    /// the frames the glow actually changed, and no others. The phase
    /// only moves on a tick, so a cockpit painting for some other
    /// reason between two ticks reuses the surface it already has.
    fn glow_phase(model: &Model) -> f32 {
        if model.theme().motion().is_full() {
            Motion::glow_phase(model.now())
        } else {
            0.0
        }
    }

    /// The rail's mini-DAG in pixels over its block — the tech-tree's
    /// caching discipline at rail size. The placed rect is reported to
    /// the model ([`Model::set_rail_graph_rect`], `None` while the rail
    /// is collapsed or graphics are off), so the view swaps between the
    /// two-line summary and the empty pixel block; a changed report
    /// makes the caller schedule the follow-up frame.
    fn show_rail_graph(
        &mut self,
        model: &mut Model,
        resolved: &Resolved,
        screen: &mut Screen,
    ) -> bool {
        let rect = resolved
            .placed()
            .iter()
            .find_map(|placed| (placed.id == Some(Widget::RailGraph.id())).then_some(placed.rect))
            .filter(|rect| rect.width >= 8 && rect.height > 0);
        let report = rect.filter(|_| screen.has_graphics());
        let report_changed = model.rail_graph_rect() != report;
        if report_changed {
            model.set_rail_graph_rect(report);
        }
        let Some(rect) = report else {
            return report_changed;
        };
        let cell = model.cell_size();
        let (nodes, edges) = model.roadmap().mini_dag(rect.width, rect.height);
        let key = (
            nodes,
            edges,
            Vec::new(),
            rect,
            cell,
            Self::glow_phase(model),
        );
        if self.rail_key.as_ref() != Some(&key) {
            let mut dag = RoadmapDag::new(key.0.clone(), key.1.clone(), (cell.width, cell.height))
                .routes(key.2.clone())
                .glow(key.5);
            if let Some(Color::Rgb(r, g, b)) = model.background() {
                dag = dag.background(r, g, b);
            }
            let surface = self
                .rail
                .get_or_insert_with(|| PixelSurface::new(rect, cell));
            surface.set_rect(rect);
            surface.set_cell_size(cell);
            surface.paint(&dag);
            self.rail_key = Some(key);
        }
        if let Some(surface) = &mut self.rail {
            screen.surface(Widget::RailGraph.id(), surface);
        }
        report_changed
    }
}

/// Where layout put `widget` this frame, when it put it anywhere with
/// room in it.
pub(super) fn placed(resolved: &Resolved, widget: Widget) -> Option<Rect> {
    placed_id(resolved, widget.id())
}

/// Where layout put the node carrying `id` this frame, when it put it
/// anywhere with room in it — the indexed widgets (a stage row, a
/// change row) whose ids are computed rather than named.
fn placed_id(resolved: &Resolved, id: WidgetId) -> Option<Rect> {
    resolved
        .placed()
        .iter()
        .find_map(|placed| (placed.id == Some(id)).then_some(placed.rect))
        .filter(|rect| !rect.is_empty())
}
