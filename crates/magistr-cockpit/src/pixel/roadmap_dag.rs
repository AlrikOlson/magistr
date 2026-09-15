//! The roadmap's tech-tree as pixels.

use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Point,
    RadialGradient, Rect as SkiaRect, Shader, SpreadMode, Stroke, Transform,
};

use crate::cockpit::dag_layout::DagLayout;
use crate::cockpit::{DagNode, DagPath};

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
/// `deps` as edges, a faint tint band per workstream (the regions), a
/// soft radial glow and a double ring on the chunk in progress — "you
/// are here" — and a bright ring on the selection.
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
        let mut edges = transitive_reduction(all_edges.clone(), nodes.len());
        // Selecting a chunk reveals its exact direct dependencies, including
        // relationships collapsed into longer paths in the overview.
        edges.extend(all_edges.into_iter().filter(|&(from, to)| {
            from < nodes.len() && to < nodes.len() && (nodes[from].selected || nodes[to].selected)
        }));
        edges.sort_unstable();
        edges.dedup();
        Self {
            nodes,
            edges,
            paths: Vec::new(),
            cell: (f32::from(cell.0), f32::from(cell.1)),
            background: Color::from_rgba8(26, 27, 38, 255),
            glow: 0.0,
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

    /// The background colour.
    #[must_use]
    pub fn background(mut self, r: u8, g: u8, b: u8) -> Self {
        self.background = Color::from_rgba8(r, g, b, 255);
        self
    }

    fn point(&self, node: &DagNode) -> (f32, f32) {
        (node.x * self.cell.0, node.y * self.cell.1)
    }
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
            if regions.iter().any(|(other, other_top, other_bottom)| {
                other != region && other_top <= bottom && other_bottom >= top
            }) {
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
        // The overview uses the transitive reduction; a selected chunk
        // also reveals every direct relationship. Longer chains stay
        // readable through their intermediate nodes.
        let stroke = Stroke {
            width: (self.cell.1 * STROKE_FRACTION).clamp(1.0, 2.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        paint.set_color(Color::from_rgba8(125, 135, 175, 95));
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
        // A dot per chunk, ringed when it needs the eye.
        for (node, &(x, y)) in self.nodes.iter().zip(&points) {
            if let Some(circle) = PathBuilder::from_circle(x, y, radius) {
                paint.set_color(status_color(&node.status));
                pixmap.fill_path(
                    &circle,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            let ring = |offset: f32| PathBuilder::from_circle(x, y, radius + offset);
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
