use magistr_cockpit::layout::PxRect;
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::lane_stop::LaneStop;
use crate::lexicon::Lexicon;
use crate::medium::Medium;

/// The volume's own graph, read off view 11's SVG (a 230×150 box): the
/// main lane at x=20 running from the bottom stop up to y=16 at
/// stroke-width 2, dots of r=4, and four stops up the lane — the
/// anchor at 130, then 90, 46 and 16.
const LANE_X: f32 = 20.0;
const LANE_TOP: f32 = 16.0;
const LANE_PX: f32 = 2.0;
const DOT_PX: f32 = 4.0;
/// The stops, bottom first. A lane holds this many steps and no more:
/// the window's tail, which is what the feed beside it shows.
pub(crate) const LANE_STOPS: [f32; 4] = [130.0, 90.0, 46.0, 16.0];
/// The focused step's ring (`r="9"`, `stroke-opacity=".35"`) and the
/// open step's dashed ring (`stroke-dasharray="3 2"`), both at the
/// SVG's default stroke of 1.
const FOCUS_RING_PX: f32 = 9.0;
const FOCUS_RING_ALPHA: f32 = 0.35;
const RING_PX: f32 = 1.0;
const OPEN_DASH: (f32, f32) = (3.0, 2.0);
/// The fork: `M20 90 C 60 90, 60 60, 100 60 L150 60` at stroke 1.5,
/// `stroke-dasharray="4 3"` — a cubic that rises 30 over its first 80
/// and a straight run to the tip at 130 right of the lane.
const FORK_PX: f32 = 1.5;
const FORK_DASH: (f32, f32) = (4.0, 3.0);
const FORK_RISE: f32 = 30.0;
const FORK_HANDLE: f32 = 40.0;
const FORK_BEND: f32 = 80.0;
const FORK_REACH: f32 = 130.0;
/// Segments the cubic is flattened into. A fixed count rather than a
/// tolerance: the curve is 80 px long at the design's density and 24
/// chords keep every one under a pixel of sagitta, which is what a
/// stamp-walked line needs to read as one curve.
const FORK_SEGMENTS: usize = 24;
/// The smallest square a dot answers a click in — the Sky's own number.
const TARGET_PX: f32 = 18.0;

/// The Trace view's branch graph, drawn from qs-gpu primitives — the same
/// primitives-not-raster route the Sky takes: the main lane, a dot per step it shows,
/// the expanded step's accent ring, the open step's dashed ring, and a dashed fork to
/// each branch tip. Nothing is rasterized, so every dot has a rect a click can land in,
/// and the two media are one drawing in two lexicons — the volume paints the graph in
/// `s1` / `ov0` / `mauve` on both.
#[derive(Clone, Debug)]
pub struct TraceGraph {
    field: PxRect,
    scale: f32,
    lex: Lexicon,
}

impl TraceGraph {
    /// A graph over `field` (physical pixels), whose design numbers are
    /// multiplied by `scale`, in `medium`'s lexicon.
    #[must_use]
    pub fn new(field: PxRect, scale: f32, medium: Medium) -> Self {
        Self {
            field,
            scale,
            lex: Lexicon::of(medium),
        }
    }

    /// Draw `stops` — bottom of the lane first — into `list`, and
    /// answer where each step's click target is: `(feed index, rect)`
    /// for every dot and fork tip that landed on the field. Stops past
    /// the lane's four are not drawn: the lane is the window's tail.
    pub fn show(&self, stops: &[LaneStop], list: &mut DrawList) -> Vec<(usize, PxRect)> {
        if self.field.width <= 0.0 || self.field.height <= 0.0 || stops.is_empty() {
            return Vec::new();
        }
        let shown = &stops[..stops.len().min(LANE_STOPS.len())];
        // The lane, from the bottom stop to the top of the box.
        let bottom = LANE_STOPS[0];
        let lane = self.lex.s1;
        list.instances.push(Instance::rect(
            self.px(LANE_X - LANE_PX * 0.5),
            self.py(LANE_TOP),
            LANE_PX * self.scale,
            (bottom - LANE_TOP) * self.scale,
            0.0,
            lane,
        ));
        let mut targets = Vec::new();
        for (stop, &y) in shown.iter().zip(&LANE_STOPS) {
            let at = (self.px(LANE_X), self.py(y));
            if let Some(tip) = stop.fork {
                self.fork(y, list);
                let tip_at = (self.px(LANE_X + FORK_REACH), self.py(y - FORK_RISE));
                self.dot(tip_at, self.lex.ov0, list);
                targets.push((tip, self.target(tip_at)));
            }
            if stop.open {
                self.dashed_ring(at, DOT_PX, self.lex.ov0, list);
            } else if stop.focused {
                self.dot(at, self.lex.mauve, list);
                let ring = self.lex.grounded(faint(self.lex.mauve, FOCUS_RING_ALPHA));
                self.ring(at, FOCUS_RING_PX, ring, list);
            } else {
                self.dot(at, lane, list);
            }
            targets.push((stop.index, self.target(at)));
        }
        targets
    }

    /// The fork off the stop at design `y`: the cubic flattened into
    /// chords and the straight run after it, walked as dashed stamps at
    /// the ink pitch (there is no line primitive and an [`Instance`]
    /// does not rotate — the Sky's own brush).
    fn fork(&self, y: f32, list: &mut DrawList) {
        let y1 = y - FORK_RISE;
        let p0 = (LANE_X, y);
        let p1 = (LANE_X + FORK_HANDLE, y);
        let p2 = (LANE_X + FORK_HANDLE, y1);
        let p3 = (LANE_X + FORK_BEND, y1);
        let mut points: Vec<(f32, f32)> = (0..=FORK_SEGMENTS)
            .map(|k| cubic(p0, p1, p2, p3, k as f32 / FORK_SEGMENTS as f32))
            .collect();
        points.push((LANE_X + FORK_REACH, y1));
        let width = FORK_PX * self.scale;
        let period = (FORK_DASH.0 + FORK_DASH.1) * self.scale;
        let inked = FORK_DASH.0 * self.scale;
        let ink = self.lex.s1;
        let mut walked = 0.0_f32;
        for pair in points.windows(2) {
            let (a, b) = (self.at(pair[0]), self.at(pair[1]));
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let length = libm::hypotf(dx, dy);
            if length <= 0.0 {
                continue;
            }
            let mut along = 0.0_f32;
            while along <= length {
                if walked.rem_euclid(period) < inked {
                    let t = along / length;
                    let at = (dx.mul_add(t, a.0), dy.mul_add(t, a.1));
                    if self.on_field(at) {
                        list.instances.push(circle(at, width * 0.5, ink));
                    }
                }
                along += width;
                walked += width;
            }
        }
    }

    fn dot(&self, at: (f32, f32), tint: Srgba, list: &mut DrawList) {
        if self.on_field(at) {
            list.instances.push(circle(at, DOT_PX * self.scale, tint));
        }
    }

    fn ring(&self, at: (f32, f32), radius_px: f32, tint: Srgba, list: &mut DrawList) {
        let r = radius_px * self.scale;
        list.instances.push(Instance::stroke(
            at.0 - r,
            at.1 - r,
            r * 2.0,
            r * 2.0,
            r,
            RING_PX * self.scale,
            tint,
        ));
    }

    fn dashed_ring(&self, at: (f32, f32), radius_px: f32, tint: Srgba, list: &mut DrawList) {
        let r = radius_px * self.scale;
        list.instances.push(Instance::dashed_stroke(
            at.0 - r,
            at.1 - r,
            r * 2.0,
            r * 2.0,
            r,
            RING_PX * self.scale,
            OPEN_DASH.0 * self.scale,
            OPEN_DASH.1 * self.scale,
            tint,
        ));
    }

    /// A design-space point on the window.
    fn at(&self, p: (f32, f32)) -> (f32, f32) {
        (self.px(p.0), self.py(p.1))
    }

    fn px(&self, x: f32) -> f32 {
        x.mul_add(self.scale, self.field.x)
    }

    fn py(&self, y: f32) -> f32 {
        y.mul_add(self.scale, self.field.y)
    }

    fn on_field(&self, at: (f32, f32)) -> bool {
        at.0 >= self.field.x
            && at.0 <= self.field.x + self.field.width
            && at.1 >= self.field.y
            && at.1 <= self.field.y + self.field.height
    }

    /// The square a dot answers a click in.
    fn target(&self, at: (f32, f32)) -> PxRect {
        let half = TARGET_PX * self.scale * 0.5;
        PxRect::new(at.0 - half, at.1 - half, half * 2.0, half * 2.0)
    }
}

/// A point on the cubic Bézier `p0 p1 p2 p3` at `t`.
fn cubic(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
        a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
    )
}

/// A filled circle: the rounded rect whose radius is its own half-width.
fn circle(at: (f32, f32), radius: f32, tint: Srgba) -> Instance {
    Instance::rect(
        at.0 - radius,
        at.1 - radius,
        radius * 2.0,
        radius * 2.0,
        radius,
        tint,
    )
}

/// `color` at `alpha`.
fn faint(color: Srgba, alpha: f32) -> Srgba {
    Srgba::new(color.r, color.g, color.b, alpha)
}

#[cfg(test)]
#[path = "trace_graph.tests.rs"]
mod tests;
