use magistr_cockpit::cockpit::{Motion, StarState};
use magistr_cockpit::layout::PxRect;
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::placed_cluster::PlacedCluster;

/// A shipped star's radius (the volume's `r="2.5"`), a live star's at
/// rest (`r="4.5"`) and how far the two live stars swell at the crest of
/// their breath — the working one to 6, the asking one to 6.5.
const SHIPPED_PX: f32 = 2.5;
const LIVE_PX: f32 = 4.5;
const WORKING_BREATH_PX: f32 = 1.5;
const ASKING_BREATH_PX: f32 = 2.0;
/// A chain's width (`stroke-width="1"`).
const EDGE_PX: f32 = 1.0;
/// The ring a selected cluster wears on its head star, and its width.
const SELECT_RING_PX: f32 = 9.0;
const RING_PX: f32 = 1.5;
/// Day's graticule paper: one hairline every this many pixels, at this
/// much of the faint ink (`.chartfield`'s 20% `#8b8aa0` on 48px).
const GRATICULE_PX: f32 = 48.0;
const GRATICULE_ALPHA: f32 = 0.2;
/// The alpha a shipped star fades to at the foot of its twinkle
/// (`@keyframes tw7`), and the alpha day's still stars sit at
/// (`.app7.latte .tw7 { opacity:.92 }`).
const TWINKLE_FLOOR: f32 = 0.45;
const DAY_ALPHA: f32 = 0.92;
/// Ticks one twinkle takes, and how far apart two neighbouring stars
/// sit in its phase — the sky's own numbers, so the two skies breathe
/// in one rhythm and never beat against each other.
const TWINKLE_STEPS: f32 = 32.0;
const TWINKLE_STRIDE: f32 = 7.0;
/// Most stamps one chain edge is drawn with — a cluster's edges are a
/// hand's width, so this never binds; it is the bound that keeps a
/// malformed layout from minting an instance per pixel.
const STAMPS_MAX: usize = 256;

/// The Galaxy's field, drawn from qs-gpu primitives: every project's cluster as stars
/// on a chain, its shipped history green, its live session breathing mauve, its pending
/// question breathing amber.
///
/// The placement is [`GalaxyLayout`](crate::galaxy_layout::GalaxyLayout)'s — the same
/// [`PlacedCluster`]s the composition pinned the labels from, so a star and its name
/// cannot come apart. Nothing is rasterized: every star, ring and edge is an instance,
/// so the zoom is applied on the frame and every cluster has a rect a click can land
/// in.
///
/// The two media are two drawings, as on the Sky: night's shipped stars twinkle and
/// day's sit still on graticule paper (ink never twinkles), and the breath — the
/// screen's one motion hero — runs on [`Motion::GLOW_STEPS`] in both. Reduced motion
/// pins every wave to its foot, which is exactly the frame a settled still draws.
#[derive(Clone, Debug)]
pub struct Galaxy {
    field: PxRect,
    scale: f32,
    lex: Lexicon,
    medium: Medium,
    now: f32,
    animate: bool,
    selected: Option<usize>,
}

impl Galaxy {
    /// A galaxy over `field` (physical pixels) whose placement is in
    /// logical pixels multiplied by `scale`, in `medium`'s lexicon.
    #[must_use]
    pub const fn new(field: PxRect, scale: f32, lex: Lexicon, medium: Medium) -> Self {
        Self {
            field,
            scale,
            lex,
            medium,
            now: 0.0,
            animate: false,
            selected: None,
        }
    }

    /// The step the field is drawn at, and whether its periodic motions
    /// run at all.
    #[must_use]
    pub const fn at(mut self, now: f32, animate: bool) -> Self {
        self.now = now;
        self.animate = animate;
        self
    }

    /// The cluster the keys walked onto, which wears a ring.
    #[must_use]
    pub const fn selecting(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Draw `clusters` into `list`, and answer where each cluster's
    /// click target is: `(project index, rect)` in physical pixels,
    /// cropped to the field.
    pub fn show(&self, clusters: &[PlacedCluster], list: &mut DrawList) -> Vec<(usize, PxRect)> {
        if self.field.width <= 0.0 || self.field.height <= 0.0 {
            return Vec::new();
        }
        self.paper(list);
        let mut targets = Vec::new();
        for (index, cluster) in clusters.iter().enumerate() {
            for &(from, to) in &cluster.edges {
                let (Some(a), Some(b)) = (cluster.stars.get(from), cluster.stars.get(to)) else {
                    continue;
                };
                self.edge(self.point(*a), self.point(*b), list);
            }
            for (star, (&at, &state)) in cluster.stars.iter().zip(&cluster.states).enumerate() {
                let at = self.point(at);
                if self.on_field(at) {
                    self.star(star, state, at, list);
                }
            }
            if self.selected == Some(index)
                && let Some(&head) = cluster.stars.last()
            {
                self.ring(
                    self.point(head),
                    SELECT_RING_PX,
                    self.alpha(self.lex.tx, 1.0),
                    list,
                );
            }
            if let Some(rect) = self.crop(cluster.bounds) {
                targets.push((index, rect));
            }
        }
        targets
    }

    /// Day's graticule paper. Night has no paper: flat ground.
    fn paper(&self, list: &mut DrawList) {
        if self.medium != Medium::Day {
            return;
        }
        let ink = self.alpha(self.lex.ov0, GRATICULE_ALPHA);
        let pitch = (GRATICULE_PX * self.scale).max(1.0);
        let width = EDGE_PX * self.scale;
        let mut x = self.field.x;
        while x < self.field.x + self.field.width {
            list.instances.push(Instance::rect(
                x,
                self.field.y,
                width,
                self.field.height,
                0.0,
                ink,
            ));
            x += pitch;
        }
        let mut y = self.field.y;
        while y < self.field.y + self.field.height {
            list.instances.push(Instance::rect(
                self.field.x,
                y,
                self.field.width,
                width,
                0.0,
                ink,
            ));
            y += pitch;
        }
    }

    /// One chain edge, walked as a brush of round stamps at the ink
    /// pitch — there is no line primitive and an [`Instance`] does not
    /// rotate.
    fn edge(&self, from: (f32, f32), to: (f32, f32), list: &mut DrawList) {
        let width = EDGE_PX * self.scale;
        let ink = self.alpha(self.lex.edge, 1.0);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = libm::hypotf(dx, dy);
        if length <= 0.0 {
            return;
        }
        let step = width;
        let stamps = ((length / step).ceil() as usize).min(STAMPS_MAX);
        for n in 0..=stamps {
            let t = (n as f32 * step / length).min(1.0);
            let at = (dx.mul_add(t, from.0), dy.mul_add(t, from.1));
            if self.on_field(at) {
                list.instances.push(circle(at, width * 0.5, ink));
            }
        }
    }

    /// One star: shipped, working or asking.
    fn star(&self, index: usize, state: StarState, at: (f32, f32), list: &mut DrawList) {
        match state {
            StarState::Executing => {
                let wave = self.phase(Motion::GLOW_STEPS as f32, 0);
                let radius = WORKING_BREATH_PX.mul_add(wave, LIVE_PX);
                list.instances.push(circle(
                    at,
                    radius * self.scale,
                    self.alpha(self.lex.mauve, 1.0),
                ));
            }
            StarState::Stuck | StarState::Blocked => {
                let wave = self.phase(Motion::GLOW_STEPS as f32, 0);
                let radius = ASKING_BREATH_PX.mul_add(wave, LIVE_PX);
                list.instances.push(circle(
                    at,
                    radius * self.scale,
                    self.alpha(self.lex.peach, 1.0),
                ));
            }
            StarState::Done | StarState::Ready | StarState::Pending | StarState::Ghost => {
                let alpha = match self.medium {
                    Medium::Night if self.animate => {
                        lerp((TWINKLE_FLOOR, 1.0), self.phase(TWINKLE_STEPS, index))
                    }
                    Medium::Night => 1.0,
                    Medium::Day => DAY_ALPHA,
                };
                list.instances.push(circle(
                    at,
                    SHIPPED_PX * self.scale,
                    self.alpha(self.lex.green, alpha),
                ));
            }
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

    /// A field-relative logical point on the window.
    fn point(&self, at: (f32, f32)) -> (f32, f32) {
        (
            self.scale.mul_add(at.0, self.field.x),
            self.scale.mul_add(at.1, self.field.y),
        )
    }

    fn on_field(&self, at: (f32, f32)) -> bool {
        at.0 >= self.field.x
            && at.0 <= self.field.x + self.field.width
            && at.1 >= self.field.y
            && at.1 <= self.field.y + self.field.height
    }

    /// A field-relative logical box on the window, cropped to the field;
    /// `None` when nothing of it is on the field.
    fn crop(&self, bounds: PxRect) -> Option<PxRect> {
        let (x0, y0) = self.point((bounds.x, bounds.y));
        let (x1, y1) = self.point((bounds.x + bounds.width, bounds.y + bounds.height));
        let left = x0.max(self.field.x);
        let top = y0.max(self.field.y);
        let right = x1.min(self.field.x + self.field.width);
        let bottom = y1.min(self.field.y + self.field.height);
        (right > left && bottom > top).then(|| PxRect::new(left, top, right - left, bottom - top))
    }

    /// Where a periodic motion of `period` ticks is, `0..1` around one
    /// raised cosine, offset by `index` stars' worth of phase. Pinned to
    /// the foot of the wave under reduced motion.
    fn phase(&self, period: f32, index: usize) -> f32 {
        if !self.animate {
            return 0.0;
        }
        let offset = (index as f32) * TWINKLE_STRIDE;
        let t = ((self.now + offset).rem_euclid(period) / period).clamp(0.0, 1.0);
        0.5 - 0.5 * libm::cosf(std::f32::consts::TAU * t)
    }

    fn alpha(&self, color: Srgba, alpha: f32) -> Srgba {
        Srgba::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
    }
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

/// `range.0` at `t == 0` and `range.1` at `t == 1`.
fn lerp(range: (f32, f32), t: f32) -> f32 {
    (range.1 - range.0).mul_add(t, range.0)
}

#[cfg(test)]
#[path = "galaxy.tests.rs"]
mod tests;
