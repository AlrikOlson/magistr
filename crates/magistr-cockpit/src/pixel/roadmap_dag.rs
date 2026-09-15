//! The roadmap's tech-tree as pixels.

use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Point,
    RadialGradient, Rect as SkiaRect, Shader, SpreadMode, Stroke, Transform,
};

use crate::cockpit::dag_layout::DagLayout;
use crate::cockpit::{DagNode, DagPath, StarState};

/// The widest a node dot gets, as a fraction of one cell's height;
/// dense trees shrink below it ([`dot_radius`]).
const DOT_FRACTION: f32 = 0.38;
/// The edge stroke's width as a fraction of one cell's height.
const STROKE_FRACTION: f32 = 0.09;
/// How far the active chunk's glow reaches, as a multiple of the dot
/// radius.
const GLOW_REACH: f32 = 3.6;
/// The glow's alpha at the dot's centre; it fades to nothing at
/// [`GLOW_REACH`].
const GLOW_ALPHA: f32 = 0.5;
/// How much further the glow reaches at the crest of its breath, as a
/// share of [`GLOW_REACH`].
const GLOW_SWELL: f32 = 0.35;
/// How much brighter the glow's core is at that crest, as a share of
/// [`GLOW_ALPHA`]. Under the swell, so the halo reads as spreading
/// rather than flashing.
const GLOW_LIFT: f32 = 0.25;
/// The alpha a dot outside the selection's focus fades to.
const DIM_ALPHA: f32 = 0.35;
/// The workstream tints cycle through these, faint over the background.
const REGION_TINTS: [[u8; 3]; 4] = [
    [122, 162, 247],
    [158, 206, 106],
    [187, 154, 247],
    [255, 158, 100],
];

/// Remove dependency edges whose reachability is already represented by
/// another path. The roadmap keeps every dependency in its model; the pixel
/// renderer draws the transitive reduction so long chains do not become a
/// hairball. Ordering is stable, and cyclic input is left intact.
fn transitive_reduction(edges: Vec<(usize, usize)>, node_count: usize) -> Vec<(usize, usize)> {
    DagLayout::reduce_edges(edges, node_count)
}

/// The tech-tree the driver paints over the Roadmap tab's graph block:
/// the chunks as status-coloured dots on their dependency grid, the
/// `deps` as routed edges, a faint tint band per workstream (the
/// regions), a soft radial glow and a double ring on the chunk in
/// progress — "you are here" — and a bright ring on the selection.
///
/// A selection is a *focus*: the selected chunk, everything it stands
/// on and everything that stands on it stay lit, the edges along those
/// chains brighten, and every unrelated dot and edge dims. An implied
/// dependency (one the overview's transitive reduction folded into a
/// longer chain) shows as that chain lighting up, never as an extra
/// line drawn through the dots between.
///
/// The inputs are what [`Roadmap`](crate::cockpit::Roadmap) derives:
/// nodes in cell coordinates (pan applied) and edges as index pairs;
/// the painter only scales cells to pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadmapDag {
    nodes: Vec<DagNode>,
    edges: Vec<(usize, usize)>,
    paths: Vec<DagPath>,
    cell: (f32, f32),
    background: Color,
    /// Where the active chunk's glow is in its breath, `0..1` around one
    /// raised cosine — [`Motion::glow_phase`](crate::cockpit::Motion::glow_phase).
    /// Zero is the foot of the wave, matching the static frame.
    glow: f32,
    /// Whether the workstream tint bands are painted. The rail's glance
    /// turns them off: a translucent band across a six-row block reads
    /// as a box, not a region.
    bands: bool,
    /// Per node, whether it is in the selection's focus — the selected
    /// chunk and everything reachable from it along dependencies in
    /// either direction. `None` while nothing is selected.
    focus: Option<Vec<bool>>,
}

impl RoadmapDag {
    /// A tech-tree of `nodes` and `edges`, scaled by a `cell` of
    /// `(width, height)` pixels, on a dark background.
    #[must_use]
    pub fn new(
        nodes: impl Into<Vec<DagNode>>,
        edges: impl Into<Vec<(usize, usize)>>,
        cell: (u16, u16),
    ) -> Self {
        let nodes = nodes.into();
        let all_edges = edges.into();
        let focus = nodes
            .iter()
            .position(|node| node.selected)
            .map(|selected| reach(selected, &all_edges, nodes.len()));
        let edges = transitive_reduction(all_edges, nodes.len());
        Self {
            nodes,
            edges,
            paths: Vec::new(),
            cell: (f32::from(cell.0), f32::from(cell.1)),
            background: Color::from_rgba8(26, 27, 38, 255),
            glow: 0.0,
            bands: true,
            focus,
        }
    }

    /// Attach routed paths produced by the shared cockpit layout.
    #[must_use]
    pub fn routes(mut self, paths: impl Into<Vec<DagPath>>) -> Self {
        self.paths = paths.into();
        self
    }

    /// The phase the active chunk's glow breathes at, `0..1` around one
    /// raised cosine. At `0` — and at `1` — the halo is exactly the
    /// static halo, which lets a settled frame be
    /// byte-identical whether or not the breath ever ran.
    #[must_use]
    pub fn glow(mut self, phase: f32) -> Self {
        self.glow = phase;
        self
    }

    /// Whether the workstream tint bands are painted.
    #[must_use]
    pub fn bands(mut self, bands: bool) -> Self {
        self.bands = bands;
        self
    }

    /// The background colour.
    #[must_use]
    pub fn background(mut self, r: u8, g: u8, b: u8) -> Self {
        self.background = Color::from_rgba8(r, g, b, 255);
        self
    }

    fn point(&self, node: &DagNode) -> (f32, f32) {
        (node.x * self.cell.0, node.y * self.cell.1)
    }

    /// Whether the node at `at` is lit: every node while nothing is
    /// selected, the focus set otherwise.
    fn is_lit(&self, at: usize) -> bool {
        self.focus.as_ref().is_none_or(|focus| focus[at])
    }
}

/// Every node reachable from `start` along `edges` in either direction,
/// `start` included — the selection's focus.
fn reach(start: usize, edges: &[(usize, usize)], count: usize) -> Vec<bool> {
    let mut seen = vec![false; count];
    let mut stack = vec![start];
    while let Some(at) = stack.pop() {
        if at >= count || seen[at] {
            continue;
        }
        seen[at] = true;
        for &(from, to) in edges {
            if from == at {
                stack.push(to);
            } else if to == at {
                stack.push(from);
            }
        }
    }
    seen
}

/// A raised cosine over one period: `0` at `t == 0` and `t == 1`, `1`
/// at the middle. The same curve [`Pen::pulse`](crate::theme::Pen::pulse)
/// runs the cell node's pen through, so the two paths breathe together.
fn wave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 - 0.5 * libm::cosf(std::f32::consts::TAU * t)
}

/// The colour of a status dot.
fn status_color(status: &str) -> Color {
    match status {
        "in_progress" => Color::from_rgba8(255, 158, 100, 255),
        "done" => Color::from_rgba8(158, 206, 106, 255),
        "blocked" => Color::from_rgba8(247, 118, 142, 255),
        "backlog" => Color::from_rgba8(86, 95, 137, 255),
        // Pending, and anything the wire grows later.
        _ => Color::from_rgba8(122, 162, 247, 255),
    }
}

/// The colour of an edge: a road walked (out of a shipped chunk) or one
/// ahead, lit or faded under a selection.
fn edge_color(walked: bool, focused: bool, lit: bool) -> Color {
    match (focused, lit, walked) {
        (true, true, _) => Color::from_rgba8(190, 200, 235, 150),
        (true, false, _) => Color::from_rgba8(125, 135, 175, 38),
        (false, _, true) => Color::from_rgba8(158, 206, 106, 105),
        (false, _, false) => Color::from_rgba8(125, 135, 175, 95),
    }
}

/// A workstream's faint tint, cycling past the palette.
fn region_tint(region: usize) -> Color {
    let [r, g, b] = REGION_TINTS[region % REGION_TINTS.len()];
    Color::from_rgba8(r, g, b, 22)
}

/// The dot radius the tree's density allows: a third of the closest
/// pair's distance, so neighbouring dots never touch, capped at the
/// comfortable size a sparse tree wears.
fn dot_radius(points: &[(f32, f32)], cell_height: f32) -> f32 {
    let cap = cell_height * DOT_FRACTION;
    let mut closest = f32::INFINITY;
    for (i, &(ax, ay)) in points.iter().enumerate() {
        for &(bx, by) in &points[i + 1..] {
            let (dx, dy) = (ax - bx, ay - by);
            closest = closest.min(dx.mul_add(dx, dy * dy));
        }
    }
    if closest.is_finite() {
        (closest.sqrt() * 0.35).clamp(1.5, cap)
    } else {
        cap
    }
}

/// The halo behind the chunk in progress: a radial gradient in the
/// in-progress colour, opaque-ish at the dot and gone by
/// [`GLOW_REACH`] radii out. `None` when the reach would be degenerate,
/// which is the only way the gradient can fail to build.
fn glow(x: f32, y: f32, radius: f32, swell: f32) -> Option<Shader<'static>> {
    let reach = radius * GLOW_REACH * GLOW_SWELL.mul_add(swell, 1.0);
    let mut core = status_color("in_progress");
    core.set_alpha((GLOW_ALPHA * GLOW_LIFT.mul_add(swell, 1.0)).min(1.0));
    let mut edge = core;
    edge.set_alpha(0.0);
    RadialGradient::new(
        Point::from_xy(x, y),
        0.0,
        Point::from_xy(x, y),
        reach,
        vec![GradientStop::new(0.0, core), GradientStop::new(1.0, edge)],
        SpreadMode::Pad,
        Transform::identity(),
    )
}

impl super::painter::Painter for RoadmapDag {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.background);
        if self.nodes.is_empty() {
            return;
        }
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        let points: Vec<(f32, f32)> = self.nodes.iter().map(|node| self.point(node)).collect();
        let radius = dot_radius(&points, self.cell.1);
        let ring_gap = (radius * 0.5).max(1.5);
        // The workstream bands: one faint tint across the width, spanning
        // the region's nodes.
        let width = pixmap.width() as f32;
        let mut regions: Vec<(usize, f32, f32)> = Vec::new();
        for (node, &(_, y)) in self.nodes.iter().zip(&points) {
            match regions
                .iter_mut()
                .find(|(region, ..)| *region == node.region)
            {
                Some((_, top, bottom)) => {
                    *top = top.min(y);
                    *bottom = bottom.max(y);
                }
                None => regions.push((node.region, y, y)),
            }
        }
        for (region, top, bottom) in &regions {
            // Connection-based placement can interleave workstreams. Do not
            // stack translucent bands over the same rows and wash out nodes.
            if !self.bands
                || regions.iter().any(|(other, other_top, other_bottom)| {
                    other != region && other_top <= bottom && other_bottom >= top
                })
            {
                continue;
            }
            let pad = radius + ring_gap + 2.0;
            if let Some(rect) = SkiaRect::from_ltrb(0.0, top - pad, width, bottom + pad) {
                paint.set_color(region_tint(*region));
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
        // The active chunk's glow, under everything the eye then lands
        // on: the halo says "here" before the ring spells it.
        for (node, &(x, y)) in self.nodes.iter().zip(&points) {
            if !node.here {
                continue;
            }
            let swell = wave(self.glow);
            let reach = radius * GLOW_REACH * GLOW_SWELL.mul_add(swell, 1.0);
            let (Some(halo), Some(shader)) = (
                PathBuilder::from_circle(x, y, reach),
                glow(x, y, radius, swell),
            ) else {
                continue;
            };
            let halo_paint = Paint {
                shader,
                anti_alias: true,
                ..Paint::default()
            };
            pixmap.fill_path(
                &halo,
                &halo_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        // The overview draws the transitive reduction, each edge along
        // its routed path. An edge out of a shipped chunk is a road
        // walked and wears a green cast; one still ahead is blue-grey.
        // Under a selection the chains through the focus brighten and
        // every other edge fades back.
        let stroke = Stroke {
            width: (self.cell.1 * STROKE_FRACTION).clamp(1.0, 2.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        for &(from, to) in &self.edges {
            let route = self
                .paths
                .iter()
                .find(|path| path.from == from && path.to == to)
                .map(|path| {
                    path.points
                        .iter()
                        .map(|&(x, y)| (x * self.cell.0, y * self.cell.1))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| match (points.get(from), points.get(to)) {
                    (Some(&from), Some(&to)) => vec![from, to],
                    _ => Vec::new(),
                });
            if route.len() < 2 {
                continue;
            }
            let walked = self
                .nodes
                .get(from)
                .is_some_and(|node| node.status == "done");
            let lit = self.is_lit(from) && self.is_lit(to);
            paint.set_color(edge_color(walked, self.focus.is_some(), lit));
            let mut path = PathBuilder::new();
            for (index, &(x1, y1)) in route.iter().enumerate() {
                if index == 0 {
                    path.move_to(x1, y1);
                    continue;
                }
                let (x2, y2) = route[index];
                let previous = route[index - 1];
                let mid = f32::midpoint(previous.0, x2);
                path.cubic_to(mid, previous.1, mid, y2, x2, y2);
            }
            if let Some(path) = path.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        // A dot per chunk, in its state's own shape: shipped and ready
        // work filled, planned work that still waits on something
        // hollow, backlog small, blocked wearing an alert ring; ringed
        // twice when it needs the eye. Everything outside the focus
        // fades back.
        for (at, (node, &(x, y))) in self.nodes.iter().zip(&points).enumerate() {
            let mut color = status_color(&node.status);
            if !self.is_lit(at) {
                color.set_alpha(DIM_ALPHA);
            }
            paint.set_color(color);
            let radius = match node.state {
                StarState::Ghost => radius * 0.5,
                _ if node.status == "backlog" => radius * 0.7,
                _ => radius,
            };
            let hollow = matches!(node.state, StarState::Pending);
            if let Some(circle) = PathBuilder::from_circle(x, y, radius) {
                if hollow {
                    let mut ground = Paint {
                        anti_alias: true,
                        ..Paint::default()
                    };
                    ground.set_color(self.background);
                    pixmap.fill_path(
                        &circle,
                        &ground,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                    if let Some(inner) =
                        PathBuilder::from_circle(x, y, (radius - stroke.width / 2.0).max(1.0))
                    {
                        pixmap.stroke_path(&inner, &paint, &stroke, Transform::identity(), None);
                    }
                } else {
                    pixmap.fill_path(
                        &circle,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            let ring = |offset: f32| PathBuilder::from_circle(x, y, radius + offset);
            if matches!(node.state, StarState::Blocked | StarState::Stuck)
                && let Some(circle) = ring(ring_gap)
            {
                pixmap.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
            }
            if node.here
                && let Some(circle) = ring(ring_gap)
            {
                paint.set_color(Color::from_rgba8(255, 158, 100, 255));
                pixmap.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
            }
            if node.selected
                && let Some(circle) = ring(ring_gap + stroke.width + 1.0)
            {
                paint.set_color(Color::from_rgba8(255, 255, 255, 255));
                pixmap.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

#[cfg(test)]
#[path = "roadmap_dag.tests.rs"]
mod tests;
