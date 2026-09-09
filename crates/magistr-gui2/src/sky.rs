use magistr_cockpit::cockpit::{DagNode, EdgeKind, Motion, ShipRun, StarState};
use magistr_cockpit::layout::PxRect;
use magistr_theme::{Palette, ThemeColor};
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::medium::Medium;

/// The design's own sizes, in the volume's pixels at
/// [`CellMetrics::DEFAULT_PX`](crate::CellMetrics::DEFAULT_PX) — every
/// one read off specimen II.b and views 03/04 rather than chosen, and
/// every one scaled by the grid so a HiDPI sky is the same picture.
///
/// A shipped star's core (`r="3"`), the working star's (`r="5"`, growing
/// to 7), the ready star's (`r="2.5"` inside a ring at 7 growing to 10),
/// the pending outline (`r="4.5"`) and the alert ring (`r="7.5"`).
const CORE_PX: f32 = 3.0;
const WORKING_CORE_PX: f32 = 5.0;
const READY_CORE_PX: f32 = 2.5;
const PENDING_RING_PX: f32 = 4.5;
const ALERT_RING_PX: f32 = 7.5;
const READY_RING_PX: f32 = 7.0;
/// How far the working core swells at the crest of its breath, and how
/// far the ready ring travels outward at the crest of its pulse.
const BREATH_PX: f32 = 2.0;
const PULSE_REACH_PX: f32 = 3.0;
/// The night halo around the working star: the ring the volume draws
/// (`r="12"`, reaching 18) and how far the light behind it carries.
const HALO_PX: f32 = 12.0;
const HALO_REACH_PX: f32 = 6.0;
const HALO_FALLOFF_PX: f32 = 10.0;
/// A dependency edge's width, a ring's width, and the dash pattern a
/// dashed ring is walked with (`stroke-dasharray="3 3"`).
const EDGE_PX: f32 = 1.0;
const RING_PX: f32 = 1.5;
const DASH_PX: f32 = 3.0;
/// The dash an edge ahead of you is drawn with (`stroke-dasharray="4 4"`).
const EDGE_DASH_PX: f32 = 4.0;
/// Day's graticule paper: one hairline every this many pixels.
const GRATICULE_PX: f32 = 48.0;
/// The smallest square a star answers a click in — a three-pixel dot is
/// a drawing, not a target.
const TARGET_PX: f32 = 18.0;

/// The alpha a shipped star fades to at the foot of its twinkle
/// (`@keyframes tw7 { 0%,100% { opacity:.45 } 50% { opacity:1 } }`).
const TWINKLE_FLOOR: f32 = 0.45;
/// The alpha a ready star's ring travels between (`.8` → `.2`), and the
/// night halo's (`.3` → `.08`).
const PULSE_ALPHA: (f32, f32) = (0.8, 0.2);
const HALO_ALPHA: (f32, f32) = (0.3, 0.08);
/// How far a pending outline and the day graticule sit from full ink.
const OUTLINE_ALPHA: f32 = 0.75;
const GRATICULE_ALPHA: f32 = 0.2;
/// A ghost star's fade — the loading placeholder, which pulses between
/// these rather than twinkling: a twinkle says "shipped".
const GHOST_ALPHA: (f32, f32) = (0.12, 0.4);

/// Ticks one twinkle takes. Deliberately not the breath's period: two motions on one
/// screen that shared a period would beat together, and the design asks the twinkle to
/// be the slow one.
const TWINKLE_STEPS: f32 = 32.0;
/// How far apart two neighbouring stars sit in the twinkle's phase.
/// Coprime with [`TWINKLE_STEPS`] so a constellation never falls into
/// step with itself — the volume gives its four twinkling stars four
/// hand-written delays for the same reason.
const TWINKLE_STRIDE: f32 = 7.0;

/// How much faster the graticule travels than the stars on a pan. Over
/// one, so the paper slides out from under the constellation and the
/// stars read as the further layer.
const PARALLAX: f32 = 1.35;

/// Ghost stars drawn while the roadmap has not arrived — a sky that is
/// blank and a sky that is loading must not look the same.
const GHOST_STARS: usize = 9;
/// The ghosts' lattice: coprime strides over the field, so nine
/// placeholders read as a constellation rather than as a grid.
const GHOST_STRIDE: (f32, f32) = (0.17, 0.11);

/// How far the ignite's flare reaches past a shipped star's core at
/// its crest — a star four times its settled size, which is what makes
/// a three-pixel dot readable as an event across a room.
const IGNITE_REACH_PX: f32 = 9.0;
/// The flare's light at night: how far past the core it carries, and
/// how soft its edge is.
const IGNITE_GLOW_PX: f32 = 22.0;
const IGNITE_FALLOFF_PX: f32 = 14.0;
/// The travelling light's own size — bigger than the hairline it runs
/// along, so the eye follows the head and not the line.
const RUN_HEAD_PX: f32 = 3.0;
/// How much of the edge the light's tail covers, as a fraction of the
/// whole segment. Short: a tail as long as the edge is a second line
/// rather than a thing moving along one.
const RUN_TAIL: f32 = 0.18;

/// Most stamps one dependency edge is drawn with. A line is a brush
/// walked along the segment (there is no line primitive and an
/// [`Instance`] does not rotate), so an unbounded walk would mint an
/// instance per pixel on a roadmap with three hundred chunks. Sized so
/// a neighbour-column road (the widest pitch, and its diagonals) stays
/// one solid line at the ink pitch; only a genuinely long cross-field
/// hop breaks into its two endpoint runs.
const STAMPS_MAX: usize = 256;

/// The roadmap as a constellation, drawn from qs-gpu primitives.
///
/// The placement is [`Roadmap`](magistr_cockpit::cockpit::Roadmap)'s own
/// — the same cell coordinates the terminal's [`RoadmapDag`] rasterizes,
/// with the camera's pan and zoom already applied by
/// [`dag_nodes_at`](magistr_cockpit::cockpit::Roadmap::dag_nodes_at).
/// What changes here is that nothing is rasterized: every star, ring,
/// halo and edge is an instance, so the camera is applied on the frame
/// rather than baked into a texture that has to be re-admitted whenever
/// it moves, and every star has a rect a click can land in.
///
/// # The two media are two drawings, not two palettes
///
/// Night's medium is light: a working star carries a real
/// [`PrimKind::Glow`](qs_gpu::frame::PrimKind::Glow) and a shipped one
/// twinkles. Day's medium is ink: the field is graticule paper, the
/// halo is a ring rather than a light, and **nothing twinkles** — which
/// is one predicate ([`StarState::twinkles`] gated on the medium) and
/// therefore something a test can assert over the draw list rather than
/// something a reviewer has to notice.
///
/// The glow is drawn *behind* a faint ring rather than instead of one.
/// A halo is [`Fidelity::Enhanced`](qs_gpu::frame::Fidelity) with a
/// floor of nothing, so the CPU tier — which is what a golden and the
/// parity gate rasterize — draws no light at all; the ring is the
/// volume's own drawing of the same halo and is exact on both tiers, so
/// the still shows what the capture shows and the window shows the
/// light as well.
#[derive(Clone, Debug)]
pub struct Sky {
    field: PxRect,
    cell: (f32, f32),
    scale: f32,
    palette: Palette,
    medium: Medium,
    now: f32,
    animate: bool,
    pan: (f32, f32),
    run: Option<ShipRun>,
    fade: f32,
}

impl Sky {
    /// A sky over `field` (physical pixels), whose placement is in
    /// `cell`-sized cells, with design sizes multiplied by `scale`.
    #[must_use]
    pub fn new(
        field: PxRect,
        cell: (f32, f32),
        scale: f32,
        palette: Palette,
        medium: Medium,
    ) -> Self {
        Self {
            field,
            cell,
            scale,
            palette,
            medium,
            now: 0.0,
            animate: false,
            pan: (0.0, 0.0),
            run: None,
            fade: 1.0,
        }
    }

    /// The step the sky is drawn at, and whether its periodic motions
    /// run at all. With `animate` off every wave is pinned to the foot
    /// it settles at, which is how reduced motion stays structural
    /// rather than being a curve nobody ticks.
    #[must_use]
    pub const fn at(mut self, now: f32, animate: bool) -> Self {
        self.now = now;
        self.animate = animate;
        self
    }

    /// The camera's pan in cells — what the graticule parallaxes
    /// against. The stars already carry it: their placement is the
    /// panned one.
    #[must_use]
    pub const fn panned(mut self, pan: (f32, f32)) -> Self {
        self.pan = pan;
        self
    }

    /// The ship moment this frame is in the middle of, if any —
    /// [`Roadmap::ship_run`](magistr_cockpit::cockpit::Roadmap::ship_run),
    /// which answers `None` on a settled sky and on a reduced-motion
    /// one. Nothing below reads `animate` for it: a run that exists is
    /// a run to draw, and a run that does not is the end frame.
    #[must_use]
    pub fn shipping(mut self, run: Option<ShipRun>) -> Self {
        self.run = run;
        self
    }

    /// The constellation at `fade` of its ink — the volume's
    /// `opacity:.5` on the field's stars and edges under a bad day
    /// (view 14). The paper is not faded: the ruling is the room's,
    /// the constellation is what has gone quiet.
    #[must_use]
    pub const fn faded(mut self, fade: f32) -> Self {
        self.fade = fade;
        self
    }

    /// Draw `nodes` and `edges` into `list`, and answer where each
    /// star's click target is: `(graph index, rect)` for every star
    /// that landed on the field.
    ///
    /// A star whose centre panned off the field is not drawn and not
    /// registered — there is no scissor on the cockpit's one batch, so
    /// a star half a screen away would paint over the rail.
    pub fn show(
        &self,
        nodes: &[DagNode],
        edges: &[(usize, usize, EdgeKind)],
        list: &mut DrawList,
    ) -> Vec<(usize, PxRect)> {
        if self.field.width <= 0.0 || self.field.height <= 0.0 {
            return Vec::new();
        }
        self.paper(list);
        if nodes.is_empty() {
            self.ghosts(list);
            return Vec::new();
        }
        let centre = self.centring(nodes);
        let point = |node: &DagNode| {
            let (x, y) = self.point(node);
            (x + centre.0, y + centre.1)
        };
        for &(from, to, kind) in edges {
            let (Some(a), Some(b)) = (nodes.get(from), nodes.get(to)) else {
                continue;
            };
            self.edge(point(a), point(b), kind, list);
            if let Some(run) = self
                .run
                .as_ref()
                .filter(|run| run.star == from && run.runs_to(to))
            {
                self.edge_run(point(a), point(b), run, list);
            }
        }
        let mut targets = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let at = point(node);
            if !self.on_field(at) {
                continue;
            }
            self.star(index, node, at, list);
            targets.push((index, self.target(at)));
        }
        targets
    }

    /// How far the whole constellation is nudged to sit in the middle
    /// of the field.
    ///
    /// The placement's pitch is capped
    /// ([`MAX_COL_PITCH`](magistr_cockpit::cockpit::Roadmap)), so a
    /// four-chunk roadmap cannot stretch to fill a window and would
    /// otherwise huddle in the top-left corner of a very large room.
    /// Only applied on the axis where the tree actually fits: past that
    /// the pan owns the axis, and a centring that moved with the camera
    /// would fight it.
    fn centring(&self, nodes: &[DagNode]) -> (f32, f32) {
        let bounds = |get: fn(&DagNode) -> f32| {
            nodes.iter().fold((f32::MAX, f32::MIN), |(lo, hi), node| {
                (lo.min(get(node)), hi.max(get(node)))
            })
        };
        // Measured on the *unpanned* placement: `dag_nodes_at` has
        // already subtracted the camera, and a centring taken from the
        // panned box would move by exactly the pan and cancel it — a
        // drag that slid the paper and left the constellation nailed
        // down.
        let (x0, x1) = bounds(|n| n.x);
        let (y0, y1) = bounds(|n| n.y);
        let (x0, x1) = (x0 + self.pan.0, x1 + self.pan.0);
        let (y0, y1) = (y0 + self.pan.1, y1 + self.pan.1);
        let axis = |lo: f32, hi: f32, cell: f32, span: f32| {
            let extent = (hi - lo) * cell;
            if extent >= span {
                return 0.0;
            }
            (span - extent) * 0.5 - lo * cell
        };
        (
            axis(x0, x1, self.cell.0, self.field.width),
            axis(y0, y1, self.cell.1, self.field.height),
        )
    }

    /// The placeholders a sky wears while its roadmap is still on the
    /// wire: [`GHOST_STARS`] outlines on a lattice, fading together.
    /// None of them is hittable — there is no chunk behind a ghost.
    fn ghosts(&self, list: &mut DrawList) {
        let alpha = lerp(GHOST_ALPHA, self.phase(Motion::GLOW_STEPS as f32, 0));
        let tint = self.ink(self.palette.overlay0, alpha);
        for star in 0..GHOST_STARS {
            let n = star as f32;
            let at = (
                self.field
                    .x
                    .mul_add(1.0, self.field.width * (0.5 + n * GHOST_STRIDE.0).fract()),
                self.field
                    .y
                    .mul_add(1.0, self.field.height * (0.5 + n * GHOST_STRIDE.1).fract()),
            );
            self.dashed_ring(at, PENDING_RING_PX, tint, list);
        }
    }

    /// Day's graticule paper, offset by the pan times [`PARALLAX`] so the ruling slides
    /// faster than the constellation on it. Night has no paper: flat ground, and the
    /// reason this emits nothing there rather than emitting it darker.
    fn paper(&self, list: &mut DrawList) {
        if self.medium != Medium::Day {
            return;
        }
        let ink = self.alpha(self.palette.overlay0, GRATICULE_ALPHA);
        let pitch = (GRATICULE_PX * self.scale).max(1.0);
        let width = EDGE_PX * self.scale;
        let (drift_x, drift_y) = self.drift();
        let mut x = self.field.x - drift_x.rem_euclid(pitch);
        while x < self.field.x + self.field.width {
            if x >= self.field.x {
                list.instances.push(Instance::rect(
                    x,
                    self.field.y,
                    width,
                    self.field.height,
                    0.0,
                    ink,
                ));
            }
            x += pitch;
        }
        let mut y = self.field.y - drift_y.rem_euclid(pitch);
        while y < self.field.y + self.field.height {
            if y >= self.field.y {
                list.instances.push(Instance::rect(
                    self.field.x,
                    y,
                    self.field.width,
                    width,
                    0.0,
                    ink,
                ));
            }
            y += pitch;
        }
    }

    /// One dependency edge, walked as a brush: [`Instance`] has no
    /// rotation and qs-gpu has no line, so a diagonal is round stamps
    /// spaced along the segment. The `ahead` kind skips the gaps, which
    /// is why the two kinds cost one function.
    ///
    /// The stamps always walk at the **ink pitch** — half the brush, so
    /// a run is a solid line. An edge longer than [`STAMPS_MAX`] stamps
    /// cover is not stretched to fit: stretching is what turned the
    /// real roadmap's long cross-workstream edges into a fog of
    /// disconnected dots. Instead the budget is split between the two
    /// ends — a run out of each star, the middle left open — which
    /// still says "these two are joined, that way" and can never read
    /// as noise.
    fn edge(&self, from: (f32, f32), to: (f32, f32), kind: EdgeKind, list: &mut DrawList) {
        let width = EDGE_PX * self.scale;
        let ink = self.ink(self.palette.surface1, 1.0);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = libm::hypotf(dx, dy);
        if length <= 0.0 {
            return;
        }
        // Touching stamps: one brush-width apart reads as a line, at
        // half the instances an overlapping walk costs.
        let step = width;
        let reach = STAMPS_MAX as f32 * step;
        let dash = EDGE_DASH_PX * self.scale;
        let mut stamp = |walked: f32| {
            // A dashed edge is ink for one dash and nothing for the
            // next — the same period the ring's dashes use, so the two
            // "not yet" marks read as one vocabulary.
            let inked = !kind.is_dashed() || (walked / dash).floor() as i64 % 2 == 0;
            if inked {
                let t = walked / length;
                let at = (dx.mul_add(t, from.0), dy.mul_add(t, from.1));
                if self.on_field(at) {
                    list.instances.push(circle(at, width * 0.5, ink));
                }
            }
        };
        if length <= reach {
            let mut walked = 0.0;
            while walked <= length {
                stamp(walked);
                walked += step;
            }
            return;
        }
        // Two endpoint runs, half the budget each, the middle open.
        let run = reach * 0.5;
        let mut walked = 0.0;
        while walked <= run {
            stamp(walked);
            stamp(length - walked);
            walked += step;
        }
    }

    /// One star, in whatever state its chunk is in.
    fn star(&self, index: usize, node: &DagNode, at: (f32, f32), list: &mut DrawList) {
        let state = node.state;
        let hue = self.hue(node);
        match state {
            StarState::Ghost => {
                let alpha = lerp(GHOST_ALPHA, self.phase(Motion::GLOW_STEPS as f32, 0));
                self.dashed_ring(
                    at,
                    PENDING_RING_PX,
                    self.ink(self.palette.overlay0, alpha),
                    list,
                );
            }
            StarState::Pending => {
                self.dashed_ring(
                    at,
                    PENDING_RING_PX,
                    self.ink(self.palette.overlay0, OUTLINE_ALPHA),
                    list,
                );
            }
            StarState::Ready => {
                let wave = self.phase(Motion::GLOW_STEPS as f32, 0);
                let radius = PULSE_REACH_PX.mul_add(wave, READY_RING_PX);
                self.ring(at, radius, self.ink(hue, lerp(PULSE_ALPHA, wave)), list);
                self.core(at, READY_CORE_PX, hue, list);
            }
            StarState::Executing | StarState::Stuck => {
                let wave = self.phase(Motion::GLOW_STEPS as f32, 0);
                self.halo(at, wave, hue, list);
                self.core(at, BREATH_PX.mul_add(wave, WORKING_CORE_PX), hue, list);
                if state.alerts() {
                    self.dashed_ring(at, ALERT_RING_PX, self.ink(self.palette.red, 1.0), list);
                }
            }
            StarState::Blocked => {
                self.core(at, READY_CORE_PX, hue, list);
                self.dashed_ring(at, ALERT_RING_PX, self.ink(self.palette.red, 1.0), list);
            }
            StarState::Done if self.igniting(index).is_some() => {
                // The star that just shipped is drawn by the moment
                // rather than by its state — see [`Sky::ignite`].
                if let Some(run) = self.igniting(index) {
                    self.ignite(at, run, list);
                }
            }
            StarState::Done => {
                // Shipped stars stay lit; at night they twinkle, each at
                // its own slow phase, and by day the same ink sits
                // still with the ring the chart gives a mark.
                let alpha = if self.twinkles(state) {
                    lerp((TWINKLE_FLOOR, 1.0), self.phase(TWINKLE_STEPS, index))
                } else {
                    1.0
                };
                if self.medium == Medium::Day {
                    self.ring(at, CORE_PX * 2.0, self.ink(hue, OUTLINE_ALPHA), list);
                }
                self.core_at(at, CORE_PX, self.ink(hue, alpha), list);
            }
        }
        if node.selected {
            self.ring(at, ALERT_RING_PX, self.ink(self.palette.text, 1.0), list);
        }
    }

    /// The run this frame is igniting `index` with, if it is the star
    /// that just shipped.
    fn igniting(&self, index: usize) -> Option<&ShipRun> {
        self.run.as_ref().filter(|run| run.star == index)
    }

    /// *Ignite*: the star that just shipped, flaring in its workstream's hue and
    /// settling to the universal shipped-green.
    ///
    /// The two media are two events, not one event in two palettes. Night's is light —
    /// a core four times its settled size with a real
    /// [`Glow`](qs_gpu::frame::PrimKind::Glow) behind it, both collapsing onto the
    /// three-pixel green dot every other shipped star wears. Day's is ink: the pen puts
    /// the mark on the page, so the settled drawing simply arrives, alpha first, with
    /// no flare at all. In both, the hue travels from the workstream's to green over
    /// the same fraction, which is what makes the moment say *which* workstream shipped
    /// without a legend.
    fn ignite(&self, at: (f32, f32), run: &ShipRun, list: &mut DrawList) {
        let t = run.ignite;
        let tint = mix(self.region_hue(run.region), self.palette.green, t);
        if self.medium == Medium::Day {
            self.ring(at, CORE_PX * 2.0, self.ink(tint, OUTLINE_ALPHA * t), list);
            self.core_at(at, CORE_PX, self.ink(tint, t), list);
            return;
        }
        let reach = 1.0 - t;
        // Nothing is emitted at the foot of the flare: a settled star
        // must draw the instances a star that never ignited draws, and
        // a transparent glow is still a glow in the list.
        if reach > 0.0 {
            let glow = IGNITE_GLOW_PX * reach * self.scale;
            list.instances.push(Instance::glow(
                at.0 - glow,
                at.1 - glow,
                glow * 2.0,
                glow * 2.0,
                glow,
                IGNITE_FALLOFF_PX * self.scale,
                self.ink(tint, reach),
            ));
        }
        self.core_at(
            at,
            IGNITE_REACH_PX.mul_add(reach, CORE_PX) * self.scale,
            self.ink(tint, 1.0),
            list,
        );
    }

    /// *Edge-run*: the light the completion sends down a dependency edge to a star it
    /// just made ready, in the hue of the workstream that caused the unlock — the
    /// unlock is signed by the work behind it.
    ///
    /// Night runs a head with a short tail: only the moving part is lit, so the eye
    /// follows it. Day draws the same travel as a pen stroke that stays on the page —
    /// everything the pen has covered remains inked — because ink does not travel and
    /// then vanish.
    fn edge_run(&self, from: (f32, f32), to: (f32, f32), run: &ShipRun, list: &mut DrawList) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = libm::hypotf(dx, dy);
        if length <= 0.0 {
            return;
        }
        let head = run.edge_run;
        // The light has not left the star yet — and a single stamp on
        // the star's own centre is not a departure, it is a smudge.
        if head <= 0.0 {
            return;
        }
        let hue = self.region_hue(run.region);
        let tail = if self.medium == Medium::Day {
            0.0
        } else {
            (head - RUN_TAIL).max(0.0)
        };
        let width = RUN_HEAD_PX * self.scale;
        let step = (length / STAMPS_MAX as f32).max(width * 0.25);
        let mut walked = tail * length;
        while walked <= head * length {
            let t = walked / length;
            let at = (dx.mul_add(t, from.0), dy.mul_add(t, from.1));
            walked += step;
            if !self.on_field(at) {
                continue;
            }
            // How near the head this stamp is, `0..1` — what the light
            // fades along and what the ink ignores.
            let along = if head > tail {
                ((t - tail) / (head - tail)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let (radius, alpha) = if self.medium == Medium::Day {
                (EDGE_PX * self.scale, 1.0)
            } else {
                (width * 0.5 * 0.4f32.mul_add(along, 0.6), along)
            };
            list.instances
                .push(circle(at, radius, self.ink(hue, alpha)));
        }
        if self.medium == Medium::Night {
            let at = (dx.mul_add(head, from.0), dy.mul_add(head, from.1));
            if self.on_field(at) {
                let glow = HALO_PX * 0.5 * self.scale;
                list.instances.push(Instance::glow(
                    at.0 - glow,
                    at.1 - glow,
                    glow * 2.0,
                    glow * 2.0,
                    glow,
                    HALO_FALLOFF_PX * self.scale,
                    self.ink(hue, HALO_ALPHA.0),
                ));
            }
        }
    }

    /// The working star's halo: the volume's ring on both tiers, and —
    /// at night only — the light behind it. See the type's own note on
    /// why the ring is not replaced by the glow.
    fn halo(&self, at: (f32, f32), wave: f32, hue: ThemeColor, list: &mut DrawList) {
        let radius = (HALO_REACH_PX.mul_add(wave, HALO_PX)) * self.scale;
        let tint = self.ink(hue, lerp(HALO_ALPHA, wave));
        if self.medium == Medium::Night {
            let falloff = HALO_FALLOFF_PX * self.scale;
            list.instances.push(Instance::glow(
                at.0 - radius,
                at.1 - radius,
                radius * 2.0,
                radius * 2.0,
                radius,
                falloff,
                tint,
            ));
        }
        list.instances.push(Instance::stroke(
            at.0 - radius,
            at.1 - radius,
            radius * 2.0,
            radius * 2.0,
            radius,
            EDGE_PX * self.scale,
            tint,
        ));
    }

    fn core(&self, at: (f32, f32), radius_px: f32, hue: ThemeColor, list: &mut DrawList) {
        self.core_at(at, radius_px, self.ink(hue, 1.0), list);
    }

    fn core_at(&self, at: (f32, f32), radius_px: f32, tint: Srgba, list: &mut DrawList) {
        list.instances
            .push(circle(at, radius_px * self.scale, tint));
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
        let dash = DASH_PX * self.scale;
        list.instances.push(Instance::dashed_stroke(
            at.0 - r,
            at.1 - r,
            r * 2.0,
            r * 2.0,
            r,
            RING_PX * self.scale,
            dash,
            dash,
            tint,
        ));
    }

    /// Where a placed node lands on the window.
    fn point(&self, node: &DagNode) -> (f32, f32) {
        (
            self.field.x + node.x * self.cell.0,
            self.field.y + node.y * self.cell.1,
        )
    }

    /// How far the paper has slid: the pan in pixels, over-travelled by
    /// [`PARALLAX`]. The stars carry the pan itself, so the difference
    /// between the two is the parallax the eye reads.
    fn drift(&self) -> (f32, f32) {
        (
            self.pan.0 * self.cell.0 * PARALLAX,
            self.pan.1 * self.cell.1 * PARALLAX,
        )
    }

    fn on_field(&self, at: (f32, f32)) -> bool {
        at.0 >= self.field.x
            && at.0 <= self.field.x + self.field.width
            && at.1 >= self.field.y
            && at.1 <= self.field.y + self.field.height
    }

    /// The square a star answers a click in — at least [`TARGET_PX`]
    /// across, cropped to nothing outside the field.
    fn target(&self, at: (f32, f32)) -> PxRect {
        let half = (TARGET_PX * self.scale) * 0.5;
        PxRect::new(at.0 - half, at.1 - half, half * 2.0, half * 2.0)
    }

    /// The workstream's hue, or the state's own colour where the state
    /// outranks the workstream: shipped is green wherever it shipped,
    /// stuck is peach and blocked is red, and nothing else is tinted at
    /// all. Workstream hues are the sky's only decoration.
    fn hue(&self, node: &DagNode) -> ThemeColor {
        match node.state {
            StarState::Done => self.palette.green,
            StarState::Stuck => self.palette.peach,
            StarState::Blocked => self.palette.red,
            StarState::Ghost | StarState::Pending => self.palette.overlay0,
            StarState::Ready | StarState::Executing => self.region_hue(node.region),
        }
    }

    /// A workstream's hue, by the ordinal the roadmap's own `group`
    /// field bands to — the ship moment's flare and the light it sends
    /// down the edge are the same colour a working star in that
    /// workstream wears, so the unlock is signed by the work.
    fn region_hue(&self, region: usize) -> ThemeColor {
        let hues = [
            self.palette.mauve,
            self.palette.blue,
            self.palette.sky,
            self.palette.lavender,
        ];
        hues[region % hues.len()]
    }

    /// Whether this state's twinkle runs: night, full motion, and a
    /// state that twinkles at all. **Ink never twinkles** — the medium
    /// is the first term, so day cannot reach the motion however the
    /// model is ticking.
    const fn twinkles(&self, state: StarState) -> bool {
        matches!(self.medium, Medium::Night) && self.animate && state.twinkles()
    }

    /// Where a periodic motion of `period` ticks is, `0..1` around one
    /// raised cosine, offset by `index` stars' worth of phase. Pinned
    /// to the foot of the wave under reduced motion, which is exactly
    /// the frame a settled still draws.
    fn phase(&self, period: f32, index: usize) -> f32 {
        if !self.animate {
            return 0.0;
        }
        let offset = (index as f32) * TWINKLE_STRIDE;
        wave((self.now + offset).rem_euclid(period) / period)
    }

    /// A constellation ink: `color` at `alpha`, through the fade.
    fn ink(&self, color: ThemeColor, alpha: f32) -> Srgba {
        self.alpha(color, alpha * self.fade)
    }

    fn alpha(&self, color: ThemeColor, alpha: f32) -> Srgba {
        Srgba::new(
            f32::from(color.r) / 255.0,
            f32::from(color.g) / 255.0,
            f32::from(color.b) / 255.0,
            alpha.clamp(0.0, 1.0),
        )
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

/// A raised cosine over one period: `0` at both ends and `1` in the
/// middle — [`Motion::glow_phase`]'s own curve, so the constellation
/// breathes with the rail's gauge rather than beside it.
fn wave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 - 0.5 * libm::cosf(std::f32::consts::TAU * t)
}

/// `from` at `t == 0` and `to` at `t == 1`, channel by channel.
fn mix(from: ThemeColor, to: ThemeColor, t: f32) -> ThemeColor {
    let channel =
        |a: u8, b: u8| (f32::from(b) - f32::from(a)).mul_add(t.clamp(0.0, 1.0), f32::from(a)) as u8;
    ThemeColor::new(
        channel(from.r, to.r),
        channel(from.g, to.g),
        channel(from.b, to.b),
    )
}

/// `range.0` at `t == 0` and `range.1` at `t == 1`.
fn lerp(range: (f32, f32), t: f32) -> f32 {
    (range.1 - range.0).mul_add(t, range.0)
}

#[cfg(test)]
#[path = "sky.tests.rs"]
mod tests;
