//! The think trace's branches as a small commit-style graph.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Stroke, Transform,
};

use super::painter::Painter;

/// The connecting stroke is this fraction of one lane's height wide.
const STROKE_FRACTION: f32 = 1.0 / 8.0;
/// A step's dot radius as a fraction of one lane's height.
const DOT_FRACTION: f32 = 1.0 / 5.0;
/// Padding above and below the lanes so dots stay inside the pixmap.
const PADDING_FRACTION: f32 = 1.0 / 10.0;
/// Padding left and right, as a fraction of the height — wide enough
/// for the selection ring around the first and the last step.
const SIDE_FRACTION: f32 = 0.45;
/// The alpha a step outside the selection's focus fades to.
const DIM_ALPHA: f32 = 0.3;
/// The pin ring's colour — the violet the pin icon wears.
const PIN_COLOR: [u8; 3] = [187, 154, 247];
/// The main line, then each branch lane in turn, wrapping.
const LANE_COLORS: [[u8; 3]; 4] = [
    [122, 162, 247],
    [255, 158, 100],
    [158, 206, 106],
    [187, 154, 247],
];

/// The trace's steps as dots on horizontal lanes — the main line on
/// lane 0, each branch on its own lane — joined along their lane and
/// linked across lanes where a branch forks, on an opaque background
/// (paint it into a [`PixelSurface::new`](super::PixelSurface) surface).
///
/// The inputs are what [`Trace`](crate::cockpit::Trace) derives: one
/// lane per step in feed order, and fork edges as index pairs. No steps
/// draw the background alone.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchGraph {
    lanes: Vec<u8>,
    links: Vec<(usize, usize)>,
    /// A merged branch's last step linked back to the main line.
    merges: Vec<(usize, usize)>,
    /// Per step, whether the selection's focus lights it; `None` while
    /// nothing is selected and every step is lit.
    focus: Option<Vec<bool>>,
    /// The selected step, ringed white.
    selected: Option<usize>,
    /// Per step, whether it is pinned — ringed violet.
    pins: Vec<bool>,
    background: Color,
    sweep: Option<f32>,
}

impl BranchGraph {
    /// A graph of the steps' `lanes` (feed order) and the fork `links`
    /// (index pairs), on a dark background.
    #[must_use]
    pub fn new(lanes: impl Into<Vec<u8>>, links: impl Into<Vec<(usize, usize)>>) -> Self {
        Self {
            lanes: lanes.into(),
            links: links.into(),
            merges: Vec::new(),
            focus: None,
            selected: None,
            pins: Vec::new(),
            background: Color::from_rgba8(26, 27, 38, 255),
            sweep: None,
        }
    }

    /// The merge links (index pairs): a merged branch's last step back
    /// to the main-line step after it, drawn in the branch's colour.
    #[must_use]
    pub fn merges(mut self, merges: impl Into<Vec<(usize, usize)>>) -> Self {
        self.merges = merges.into();
        self
    }

    /// The selection: which steps its focus lights (`None` lights all)
    /// and which step wears the selection ring.
    #[must_use]
    pub fn focus(mut self, lit: Option<Vec<bool>>, selected: Option<usize>) -> Self {
        self.focus = lit;
        self.selected = selected;
        self
    }

    /// Which steps are pinned, per feed index.
    #[must_use]
    pub fn pins(mut self, pins: impl Into<Vec<bool>>) -> Self {
        self.pins = pins.into();
        self
    }

    /// Whether the step at `index` is lit by the focus.
    fn is_lit(&self, index: usize) -> bool {
        self.focus
            .as_ref()
            .is_none_or(|focus| focus.get(index).copied().unwrap_or(false))
    }

    /// One frame of the sweep that draws the graph in from the left:
    /// only the first `t` of the width (`0..1`) is revealed. `None` — or
    /// `1` and beyond — draws it whole; `0` draws nothing but the
    /// background. The frame is a pure function of `t`, so the runtime's
    /// tick can step it.
    ///
    /// The same meaning as [`Sparkline::sweep`](super::Sparkline::sweep),
    /// with one difference the shape forces: a step is a discrete thing,
    /// so the cut falls between steps rather than interpolating a head
    /// onto the edge. A step is either revealed or it is not, and the
    /// lines and fork edges into it come with it.
    #[must_use]
    pub fn sweep(mut self, t: Option<f32>) -> Self {
        self.sweep = t.map(|t| t.clamp(0.0, 1.0)).filter(|t| *t < 1.0);
        self
    }

    /// The background colour.
    #[must_use]
    pub fn background(mut self, r: u8, g: u8, b: u8) -> Self {
        self.background = Color::from_rgba8(r, g, b, 255);
        self
    }

    /// The steps' lanes.
    #[must_use]
    pub fn lanes(&self) -> &[u8] {
        &self.lanes
    }

    /// Each step's pixel position: x spread over the width in feed
    /// order, y at the middle of its lane.
    fn points(&self, width: f32, height: f32) -> Vec<(f32, f32)> {
        let lane_count = f32::from(self.lanes.iter().copied().max().unwrap_or(0)) + 1.0;
        let padding = (height * PADDING_FRACTION).max(1.0);
        let side = (height * SIDE_FRACTION).max(1.0).min(width / 4.0);
        let lane_height = (height - 2.0 * padding) / lane_count;
        let step = if self.lanes.len() > 1 {
            (width - 2.0 * side) / (self.lanes.len() - 1) as f32
        } else {
            0.0
        };
        self.lanes
            .iter()
            .enumerate()
            .map(|(index, &lane)| {
                let x = if self.lanes.len() > 1 {
                    side + index as f32 * step
                } else {
                    width / 2.0
                };
                (x, padding + (f32::from(lane) + 0.5) * lane_height)
            })
            .collect()
    }

    /// `points` cut at the sweep's edge — every step past it dropped,
    /// so the lane runs, the fork edges and the dots that need it all
    /// fall away together (each is indexed against this list) — or all
    /// of them when there is no sweep.
    fn swept(&self, points: Vec<(f32, f32)>, width: f32) -> Vec<(f32, f32)> {
        let Some(t) = self.sweep else {
            return points;
        };
        let edge = t * width;
        points.into_iter().take_while(|p| p.0 <= edge).collect()
    }

    fn lane_height(&self, height: f32) -> f32 {
        let lane_count = f32::from(self.lanes.iter().copied().max().unwrap_or(0)) + 1.0;
        let padding = (height * PADDING_FRACTION).max(1.0);
        (height - 2.0 * padding) / lane_count
    }
}

/// The colour of a lane, wrapping past the palette.
fn lane_color(lane: u8) -> Color {
    let [r, g, b] = LANE_COLORS[usize::from(lane) % LANE_COLORS.len()];
    Color::from_rgba8(r, g, b, 255)
}

impl Painter for BranchGraph {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.background);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let points = self.swept(self.points(width, height), width);
        if points.is_empty() {
            return;
        }
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        let stroke = Stroke {
            width: (self.lane_height(height) * STROKE_FRACTION).clamp(1.0, 3.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        // Along each lane: the consecutive steps joined.
        let lane_count = self.lanes.iter().copied().max().unwrap_or(0);
        for lane in 0..=lane_count {
            // Zipping against the (possibly swept) points is what drops
            // everything past the edge: a lane run, a dot and a fork
            // edge are all indexed against this list.
            let on_lane: Vec<(f32, f32)> = self
                .lanes
                .iter()
                .zip(&points)
                .filter(|(l, _)| **l == lane)
                .map(|(_, point)| *point)
                .collect();
            if on_lane.len() < 2 {
                continue;
            }
            let mut path = PathBuilder::new();
            path.move_to(on_lane[0].0, on_lane[0].1);
            for &(x, y) in &on_lane[1..] {
                path.line_to(x, y);
            }
            if let Some(path) = path.finish() {
                paint.set_color(lane_color(lane));
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        // The fork edges in the branch's colour, and a merged branch's
        // way back to the main line in its own colour too: a fork wears
        // the lane it forks *to*, a merge the lane it comes *from*.
        let forks = self.links.iter().map(|&(from, to)| (from, to, to));
        let merges = self.merges.iter().map(|&(from, to)| (from, to, from));
        for (from, to, coloured_by) in forks.chain(merges) {
            let (Some(&(x1, y1)), Some(&(x2, y2))) = (points.get(from), points.get(to)) else {
                continue;
            };
            let mut path = PathBuilder::new();
            path.move_to(x1, y1);
            path.line_to(x2, y2);
            if let (Some(path), Some(&lane)) = (path.finish(), self.lanes.get(coloured_by)) {
                paint.set_color(lane_color(lane));
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        // A dot per step over the lines — faded outside the focus, the
        // newest a little larger, a pinned one ringed violet and the
        // selected one ringed white.
        let radius = (self.lane_height(height) * DOT_FRACTION).max(1.5);
        let newest = points.len().saturating_sub(1);
        for (index, (&lane, &(x, y))) in self.lanes.iter().zip(&points).enumerate() {
            let mut colour = lane_color(lane);
            if !self.is_lit(index) {
                colour.set_alpha(DIM_ALPHA);
            }
            let dot = if index == newest && self.sweep.is_none() {
                radius * 1.3
            } else {
                radius
            };
            if let Some(circle) = PathBuilder::from_circle(x, y, dot) {
                paint.set_color(colour);
                pixmap.fill_path(
                    &circle,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            let ring_gap = (radius * 0.6).max(1.5);
            if self.pins.get(index).copied().unwrap_or(false)
                && let Some(ring) = PathBuilder::from_circle(x, y, dot + ring_gap)
            {
                let [r, g, b] = PIN_COLOR;
                let mut pin = Color::from_rgba8(r, g, b, 255);
                if !self.is_lit(index) {
                    pin.set_alpha(DIM_ALPHA);
                }
                paint.set_color(pin);
                pixmap.stroke_path(&ring, &paint, &stroke, Transform::identity(), None);
            }
            if self.selected == Some(index)
                && let Some(ring) =
                    PathBuilder::from_circle(x, y, dot + ring_gap + stroke.width + 1.0)
            {
                paint.set_color(Color::from_rgba8(255, 255, 255, 255));
                pixmap.stroke_path(&ring, &paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

#[cfg(test)]
#[path = "branch_graph.tests.rs"]
mod tests;
