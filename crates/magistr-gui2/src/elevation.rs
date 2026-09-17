//! How far off the canvas a surface stands, and what that looks like.

use magistr_theme::{Palette, ThemeColor, math};
use qs_gpu::color::Srgba;
use qs_gpu::frame::Instance;

/// Surface depth in the native elevation scale: canvas 0, row 1, raised 5, overlay 10,
/// and popover 16.
///
/// Drawing combines a surface-lightness shift, two soft shadows, and an inset rim. The
/// contact shadow anchors the surface; the broader shadow expresses its distance from
/// the background. The rim is part of the fill rather than a one-pixel border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elevation {
    /// The design language's own number, in its own units.
    level: f32,
}

impl Elevation {
    /// The popover level, and therefore the scale's top — every
    /// treatment here is a share of it.
    const TOP: f32 = 16.0;

    /// How much of the palette's own one-level step the top of the scale
    /// moves a ground by.
    ///
    /// A share rather than a colour, so a user palette with a gentler
    /// `base`→`surface0` step gets a gentler lift and one with a bolder
    /// step gets a bolder one — and under 1.0 because a popover that
    /// moved a *full* step would land exactly on `surface0` and stop
    /// being distinguishable from the panels already painted in it.
    const LIFT: f32 = 0.75;

    /// The contact shadow's alpha at the top of the scale, and the
    /// ambient one's. The contact shadow is the darker and much the
    /// tighter of the two.
    const CONTACT_ALPHA: f32 = 0.28;
    const AMBIENT_ALPHA: f32 = 0.14;

    /// The rim's alpha at the top of the scale, and how far it reaches
    /// inward in logical pixels. Small on purpose: a rim that reached
    /// into the surface would change the ground under a label without
    /// the sweep being able to see it.
    const RIM_ALPHA: f32 = 0.10;
    const RIM_WIDTH: f32 = 1.0;

    /// The scale's *raised* step (5) with no cockpit surface kind of its own: what an
    /// action button stands at. The design gives a button depth a surface kind cannot
    /// carry — a button is not a pane — so the painter asks for the level by name.
    #[must_use]
    pub const fn raised() -> Self {
        Self { level: 5.0 }
    }

    /// The canvas itself — level 0, no depth to draw.
    ///
    /// These four named levels (with [`Elevation::raised`]) are scale spoken without a
    /// cockpit surface kind, for the element substrate: its tree owes the cockpit's
    /// layout vocabulary nothing, so it asks for a depth by the design's own name.
    #[must_use]
    pub const fn canvas() -> Self {
        Self { level: 0.0 }
    }

    /// A row's lift — level 1, the moment a surface stops being the
    /// canvas.
    #[must_use]
    pub const fn row() -> Self {
        Self { level: 1.0 }
    }

    /// An overlay's lift — level 10, between a raised pane and a popover.
    #[must_use]
    pub const fn overlay() -> Self {
        Self { level: 10.0 }
    }

    /// The popover's lift — the top of the scale.
    #[must_use]
    pub const fn popover() -> Self {
        Self { level: Self::TOP }
    }

    /// Whether this surface is on the canvas, and therefore has no
    /// depth to draw at all.
    #[must_use]
    pub fn is_flat(&self) -> bool {
        self.level <= 0.0
    }

    /// How far up the scale this is, `0..=1` — the perceptual share, so
    /// the first step off the canvas is worth more than the last.
    ///
    /// A square root rather than the ratio: 1 against 16 is a twentieth
    /// of a shadow, which is no shadow at all, where a row genuinely is
    /// the moment a surface stops being the canvas.
    fn share(&self) -> f32 {
        (self.level / Self::TOP).clamp(0.0, 1.0).sqrt()
    }

    /// `band` as it is actually painted at this elevation: moved
    /// **lighter** by a share of the palette's own `base`→`surface0`
    /// step.
    ///
    /// Lighter in both media, deliberately: the blessed captures float
    /// their day cards *toward white* on the grey page (view 05's edit
    /// card, the composer pill), where Latte's own surface ramp runs
    /// darker — so the palette contributes the step's *size* and the
    /// design contributes its direction. A palette whose ramp already
    /// lightens (Mocha) is untouched by the flip.
    ///
    /// The one treatment here that is not decoration — this is the
    /// colour the painter threads down as the ground, and therefore the
    /// colour every contrast question on the surface is asked against.
    #[must_use]
    pub fn ground(&self, band: Srgba, palette: &Palette) -> Srgba {
        if self.is_flat() {
            return band;
        }
        let step = self.share() * Self::LIFT;
        let (base, up) = (channels(palette.base), channels(palette.surface0));
        let sign = if Self::lifts_lighter(palette) {
            1.0
        } else {
            -1.0
        };
        Srgba::new(
            (band.r + (up[0] - base[0]) * step * sign).clamp(0.0, 1.0),
            (band.g + (up[1] - base[1]) * step * sign).clamp(0.0, 1.0),
            (band.b + (up[2] - base[2]) * step * sign).clamp(0.0, 1.0),
            band.a,
        )
    }

    /// The two shadow layers under a surface of `rect` (physical pixels)
    /// with corner `radius`, at a grid scale of `scale`.
    ///
    /// Both are [`PrimKind::Glow`](qs_gpu::frame::PrimKind::Glow), which
    /// draws the shape solid inside a halo that fades over its falloff —
    /// so they are pushed *before* the band, which then covers the solid
    /// part and leaves only the halo showing. Offset downward, which is
    /// the direction the framework's own lighting rig puts the key light
    /// from.
    ///
    /// Empty on the canvas: a flat surface has nothing to cast.
    #[must_use]
    pub fn shadows(&self, rect: [f32; 4], radius: f32, scale: f32) -> Vec<Instance> {
        if self.is_flat() {
            return Vec::new();
        }
        let [x, y, w, h] = rect;
        let depth = self.level * scale;
        let share = self.share();
        let layer = |drop: f32, falloff: f32, alpha: f32| {
            Instance::glow(
                x,
                y + drop,
                w,
                h,
                radius,
                falloff.max(1.0),
                Srgba::new(0.0, 0.0, 0.0, alpha * share),
            )
        };
        vec![
            // Ambient first, so the contact shadow sits over it: a
            // surface is darkest where it nearly touches.
            layer(depth * 0.5, depth * 1.5, Self::AMBIENT_ALPHA),
            layer(depth * 0.15, depth * 0.4, Self::CONTACT_ALPHA),
        ]
    }

    /// The lit edge over a surface of `rect` with corner `radius`, or
    /// `None` on the canvas.
    ///
    /// Drawn *after* the band and before the surface's content, so the
    /// light is on the surface rather than under it.
    #[must_use]
    pub fn rim(&self, rect: [f32; 4], radius: f32, scale: f32) -> Option<Instance> {
        if self.is_flat() {
            return None;
        }
        let [x, y, w, h] = rect;
        Some(Instance::rim(
            x,
            y,
            w,
            h,
            radius,
            Self::RIM_WIDTH * scale,
            Srgba::new(1.0, 1.0, 1.0, Self::RIM_ALPHA * self.share()),
        ))
    }

    /// Whether this palette's own surface ramp lightens — Mocha
    /// does, Latte darkens. [`Elevation::ground`] reads it to keep its
    /// lift pointed lighter whichever way the ramp runs — the blessed
    /// captures' own direction in both media.
    #[must_use]
    pub fn lifts_lighter(palette: &Palette) -> bool {
        math::relative_luminance(palette.surface0) > math::relative_luminance(palette.base)
    }
}

/// A theme colour's three channels in `0..=1`.
fn channels(colour: ThemeColor) -> [f32; 3] {
    [
        f32::from(colour.r) / 255.0,
        f32::from(colour.g) / 255.0,
        f32::from(colour.b) / 255.0,
    ]
}

#[cfg(test)]
#[path = "elevation.tests.rs"]
mod tests;
