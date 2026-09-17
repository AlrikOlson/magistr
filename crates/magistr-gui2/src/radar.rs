use magistr_cockpit::layout::PxRect;
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::lexicon::Lexicon;
use crate::medium::Medium;

/// The survey sweep: the `.radar` disc — 1400 px across, centred on the field —
/// carrying a 60° wedge of the accent's glow that fades from its leading edge to
/// nothing (`conic-gradient(from 0deg, tint 0deg, transparent 60deg)`) and turns once
/// every [`Birth::RADAR_STEPS`]; and the faint points the sweep passes over, code the
/// survey noticed that will inform the charting.
///
/// The wedge is rasterized as rows of axis-aligned runs, each run at the alpha of its
/// angle — an instance does not rotate and the framework's own sweep primitive ramps
/// around the whole turn, where the volume's wedge is a sixth of one. The same runs on
/// both tiers, so the still a golden rasterizes is the still the window draws. The
/// phase is the cockpit's [`Birth::radar_phase`], a pure function of the tick, so
/// reduced motion holds the sweep at its start.
///
/// [`Birth::RADAR_STEPS`]: magistr_cockpit::cockpit::Birth::RADAR_STEPS
/// [`Birth::radar_phase`]: magistr_cockpit::cockpit::Birth::radar_phase
#[derive(Clone, Debug)]
pub struct Radar {
    field: PxRect,
    scale: f32,
    lex: Lexicon,
    medium: Medium,
    phase: f32,
}

/// The disc's diameter in design pixels (`.radar { width:1400px }`).
const DISC_PX: f32 = 1400.0;
/// The wedge's span — `transparent 60deg`.
const SPAN: f32 = std::f32::consts::TAU / 6.0;
/// The glow's share at the sweep's leading edge: night mixes the accent
/// glow at 13%, day the ink at 24% (`.app7.latte .radar`).
const NIGHT_ALPHA: f32 = 0.13;
const DAY_ALPHA: f32 = 0.24;
/// Day's tint — the volume's `#8b8aa0`.
const DAY_TINT: Srgba = Srgba::new(
    0x8b as f32 / 255.0,
    0x8a as f32 / 255.0,
    0xa0 as f32 / 255.0,
    1.0,
);
/// A survey point's radius (`r="2"`).
const POINT_PX: f32 = 2.0;
/// The wedge's raster: rows this many design pixels tall, sampled
/// this many design pixels apart along each, and the fade quantized
/// to this many steps — fine enough that no step reads as a band.
const ROW_PX: f32 = 2.0;
const SAMPLE_PX: f32 = 4.0;
const STEPS: f32 = 12.0;

impl Radar {
    /// A sweep over `field` (physical pixels) at `scale`, in `medium`'s
    /// lexicon.
    #[must_use]
    pub const fn new(field: PxRect, scale: f32, lex: Lexicon, medium: Medium) -> Self {
        Self {
            field,
            scale,
            lex,
            medium,
            phase: 0.0,
        }
    }

    /// Where the sweep's leading edge points, `0..1` of a turn
    /// clockwise from the top.
    #[must_use]
    pub const fn at(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }

    /// Draw the wedge and `points` (thousandths of the field) into
    /// `list`.
    pub fn show(&self, points: &[(u16, u16)], list: &mut DrawList) {
        if self.field.width <= 0.0 || self.field.height <= 0.0 {
            return;
        }
        self.wedge(list);
        let ink = self.lex.s1;
        let r = POINT_PX * self.scale;
        for &(px, py) in points {
            let x = self.field.x + self.field.width * f32::from(px) / 1000.0;
            let y = self.field.y + self.field.height * f32::from(py) / 1000.0;
            list.instances
                .push(Instance::rect(x - r, y - r, r * 2.0, r * 2.0, r, ink));
        }
    }

    /// The wedge, row by row: every run of samples at one quantized
    /// alpha becomes one rect.
    fn wedge(&self, list: &mut DrawList) {
        let (tint, peak) = match self.medium {
            Medium::Night => (self.lex.glow_m, NIGHT_ALPHA),
            Medium::Day => (DAY_TINT, DAY_ALPHA),
        };
        let cx = self.field.x + self.field.width * 0.5;
        let cy = self.field.y + self.field.height * 0.5;
        let radius = DISC_PX * self.scale * 0.5;
        let lead = self.phase.rem_euclid(1.0) * std::f32::consts::TAU;
        let row = (ROW_PX * self.scale).max(1.0);
        let sample = (SAMPLE_PX * self.scale).max(1.0);
        let top = self.field.y.max(cy - radius);
        let bottom = (self.field.y + self.field.height).min(cy + radius);
        let mut y = top;
        while y < bottom {
            let ym = y + row * 0.5;
            let mut x = self.field.x;
            let end = self.field.x + self.field.width;
            let mut run: Option<(f32, u32)> = None;
            while x < end {
                let xm = x + sample * 0.5;
                let level = self.level(xm - cx, ym - cy, radius, lead);
                match run {
                    Some((_, l)) if l == level => {}
                    Some((from, l)) => {
                        self.push(list, from, x, y, row, l, tint, peak);
                        run = Some((x, level));
                    }
                    None => run = Some((x, level)),
                }
                x += sample;
            }
            if let Some((from, l)) = run {
                self.push(list, from, end, y, row, l, tint, peak);
            }
            y += row;
        }
    }

    /// The fade level at `(dx, dy)` from the centre: `0` outside the
    /// wedge, up to [`STEPS`] at its leading edge.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn level(&self, dx: f32, dy: f32, radius: f32, lead: f32) -> u32 {
        if dx * dx + dy * dy > radius * radius {
            return 0;
        }
        // Clockwise from the top, the way a conic gradient counts.
        let angle = libm::atan2f(dx, -dy).rem_euclid(std::f32::consts::TAU);
        let rel = (angle - lead).rem_euclid(std::f32::consts::TAU);
        if rel >= SPAN {
            return 0;
        }
        ((1.0 - rel / SPAN) * STEPS).ceil() as u32
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &self,
        list: &mut DrawList,
        from: f32,
        to: f32,
        y: f32,
        row: f32,
        level: u32,
        tint: Srgba,
        peak: f32,
    ) {
        if level == 0 || to <= from {
            return;
        }
        let alpha = peak * level as f32 / STEPS;
        list.instances.push(Instance::rect(
            from,
            y,
            to - from,
            row,
            0.0,
            Srgba::new(tint.r, tint.g, tint.b, alpha),
        ));
    }
}

#[cfg(test)]
#[path = "radar.tests.rs"]
mod tests;
