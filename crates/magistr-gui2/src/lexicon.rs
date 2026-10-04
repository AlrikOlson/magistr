use qs_gpu::color::Srgba;

use crate::medium::Medium;

/// The volume's own palette for one [`Medium`] — the `.app7` /
/// `.app7.latte` custom properties, generated off the blessed volume by
/// `scripts/almagest-tokens.py` into `design/almagest7_palette.rs`.
///
/// The control atoms paint in this and never in a Catppuccin slot: the
/// specimen captures are pictures of *these* hexes, so an atom's parity
/// against them is only meaningful if the atom reads the same table the
/// browser did. `hairc` is the one non-hex var the volume declares — a
/// `color-mix(in oklab, C p%, transparent)`, which is C at alpha p —
/// and it arrives here with its alpha already on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lexicon {
    /// The window's ground.
    pub base: Srgba,
    /// The recessed ground (wells, the meter's track bed).
    pub deep: Srgba,
    /// The lifted neutral ground.
    pub mantle: Srgba,
    /// The first surface step — flat control faces.
    pub s0: Srgba,
    /// The second surface step — the switch's off track.
    pub s1: Srgba,
    /// Faint ink: separators, disabled copy.
    pub ov0: Srgba,
    /// The text ink.
    pub tx: Srgba,
    /// Subdued ink, first step.
    pub sb0: Srgba,
    /// Subdued ink, second step.
    pub sb1: Srgba,
    /// The blue state code.
    pub blue: Srgba,
    /// The accent — live / attention.
    pub mauve: Srgba,
    /// The green state code — success, the primary action.
    pub green: Srgba,
    /// The yellow state code — warnings, the meter's breaker tick.
    pub yellow: Srgba,
    /// The red state code — danger.
    pub red: Srgba,
    /// The peach state code.
    pub peach: Srgba,
    /// The teal state code.
    pub teal: Srgba,
    /// The mauve glow's color.
    pub glow_m: Srgba,
    /// The green glow's color.
    pub glow_g: Srgba,
    /// The peach glow's color.
    pub glow_p: Srgba,
    /// The window's edge line.
    pub edge: Srgba,
    /// The hairline — its alpha is part of the token.
    pub hairc: Srgba,
    /// The sixteen ANSI colours a program's own `SGR 30–37` / `90–97` paint as on this
    /// medium, in SGR order — black, red, green, yellow, blue, magenta, cyan, white,
    /// then the same eight bright. The volume's own tokens in the shape
    /// `magistr_theme::ansi16` gives a palette: black is the first surface step and
    /// bright black the second, white the second subdued ink and bright white the text;
    /// the six hues are the medium's state codes, magenta the accent (mauve) and cyan
    /// the teal, since the volume carries no sky; the bright hues are the same six. The
    /// one place hue is data.
    pub ansi: [Srgba; 16],
}

// The generated table. Allowed dead code for the same reason the voice
// oracle is: it is the volume's vocabulary, not this crate's, and
// trimming it to today's call sites would make it a list of what we
// have got round to rather than a reading of the volume.
#[allow(dead_code)]
mod oracle {
    include!("../design/almagest7_palette.rs");
}

impl Lexicon {
    /// Resolve a shared text color in this native palette.
    pub(crate) fn text_color(&self, color: magistr_cockpit::render::Color) -> Option<Srgba> {
        use magistr_cockpit::render::Color;
        let rgb = |c: magistr_theme::ThemeColor| {
            Srgba::new(
                f32::from(c.r) / 255.0,
                f32::from(c.g) / 255.0,
                f32::from(c.b) / 255.0,
                1.0,
            )
        };
        match color {
            Color::Default => None,
            Color::Indexed(n) if n < 16 => Some(self.ansi[usize::from(n)]),
            Color::Indexed(n) => Some(rgb(magistr_theme::xterm256(n))),
            Color::Rgb(r, g, b) => Some(rgb(magistr_theme::ThemeColor::new(r, g, b))),
        }
    }
    /// Keep authored text colors readable against their native surface.
    pub(crate) fn readable(ink: Srgba, on: Srgba, minimum: f64) -> Srgba {
        let rgb = |c: Srgba| {
            magistr_theme::ThemeColor::new(
                (c.r * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.b * 255.0).round() as u8,
            )
        };
        let c = magistr_theme::math::ensure_contrast(rgb(ink), rgb(on), minimum);
        Srgba::new(
            f32::from(c.r) / 255.0,
            f32::from(c.g) / 255.0,
            f32::from(c.b) / 255.0,
            1.0,
        )
    }
    /// `tint` composited onto `ground` **in sRGB bytes** — the space
    /// the browser blends the volume's translucent tokens in.
    ///
    /// The CPU tier composites in linear light, which is physically
    /// right and visibly *lighter* than what the captures show for a
    /// 40% hairline on the night ground (the live chip's ring measures
    /// exactly the sRGB blend). A token that always sits on a known
    /// ground is therefore precomposed here and painted opaque, so the
    /// pixel the atom puts down is the pixel the capture holds.
    #[must_use]
    pub fn composite(ground: Srgba, tint: Srgba) -> Srgba {
        let over = |g: f32, t: f32| {
            let byte = |v: f32| (v * 255.0).round();
            (byte(g) + (byte(t) - byte(g)) * tint.a).round() / 255.0
        };
        Srgba::new(
            over(ground.r, tint.r),
            over(ground.g, tint.g),
            over(ground.b, tint.b),
            1.0,
        )
    }

    /// `color` as atlas ink bytes — the key a [`Mark`](crate::Mark)
    /// raster is admitted under, so two inks a rounding error apart
    /// stay one picture.
    #[must_use]
    pub fn ink_bytes(color: Srgba) -> (u8, u8, u8) {
        let byte = |v: f32| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let b = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            b
        };
        (byte(color.r), byte(color.g), byte(color.b))
    }

    /// `tint` composited onto the palette background for chip rings.
    #[must_use]
    pub fn grounded(&self, tint: Srgba) -> Srgba {
        Self::composite(self.base, tint)
    }

    /// The palette `medium` paints in.
    #[must_use]
    pub fn of(medium: Medium) -> Self {
        match medium {
            Medium::Night => Self {
                base: ink(oracle::NIGHT_BASE),
                deep: ink(oracle::NIGHT_DEEP),
                mantle: ink(oracle::NIGHT_MANTLE),
                s0: ink(oracle::NIGHT_S0),
                s1: ink(oracle::NIGHT_S1),
                ov0: ink(oracle::NIGHT_OV0),
                tx: ink(oracle::NIGHT_TX),
                sb0: ink(oracle::NIGHT_SB0),
                sb1: ink(oracle::NIGHT_SB1),
                blue: ink(oracle::NIGHT_BLUE),
                mauve: ink(oracle::NIGHT_MAUVE),
                green: ink(oracle::NIGHT_GREEN),
                yellow: ink(oracle::NIGHT_YELLOW),
                red: ink(oracle::NIGHT_RED),
                peach: ink(oracle::NIGHT_PEACH),
                teal: ink(oracle::NIGHT_TEAL),
                glow_m: ink(oracle::NIGHT_GLOW_M),
                glow_g: ink(oracle::NIGHT_GLOW_G),
                glow_p: ink(oracle::NIGHT_GLOW_P),
                edge: ink(oracle::NIGHT_EDGE),
                hairc: faint(oracle::NIGHT_HAIRC, oracle::NIGHT_HAIRC_ALPHA),
                ansi: ansi(
                    [oracle::NIGHT_S0, oracle::NIGHT_S1],
                    [oracle::NIGHT_SB1, oracle::NIGHT_TX],
                    [
                        oracle::NIGHT_RED,
                        oracle::NIGHT_GREEN,
                        oracle::NIGHT_YELLOW,
                        oracle::NIGHT_BLUE,
                        oracle::NIGHT_MAUVE,
                        oracle::NIGHT_TEAL,
                    ],
                ),
            },
            Medium::Day => Self {
                base: ink(oracle::DAY_BASE),
                deep: ink(oracle::DAY_DEEP),
                mantle: ink(oracle::DAY_MANTLE),
                s0: ink(oracle::DAY_S0),
                s1: ink(oracle::DAY_S1),
                ov0: ink(oracle::DAY_OV0),
                tx: ink(oracle::DAY_TX),
                sb0: ink(oracle::DAY_SB0),
                sb1: ink(oracle::DAY_SB1),
                blue: ink(oracle::DAY_BLUE),
                mauve: ink(oracle::DAY_MAUVE),
                green: ink(oracle::DAY_GREEN),
                yellow: ink(oracle::DAY_YELLOW),
                red: ink(oracle::DAY_RED),
                peach: ink(oracle::DAY_PEACH),
                teal: ink(oracle::DAY_TEAL),
                glow_m: ink(oracle::DAY_GLOW_M),
                glow_g: ink(oracle::DAY_GLOW_G),
                glow_p: ink(oracle::DAY_GLOW_P),
                edge: ink(oracle::DAY_EDGE),
                hairc: faint(oracle::DAY_HAIRC, oracle::DAY_HAIRC_ALPHA),
                ansi: ansi(
                    [oracle::DAY_S0, oracle::DAY_S1],
                    [oracle::DAY_SB1, oracle::DAY_TX],
                    [
                        oracle::DAY_RED,
                        oracle::DAY_GREEN,
                        oracle::DAY_YELLOW,
                        oracle::DAY_BLUE,
                        oracle::DAY_MAUVE,
                        oracle::DAY_TEAL,
                    ],
                ),
            },
        }
    }
}

/// The sixteen ANSI colours from a medium's tokens: `blacks` are the
/// normal and bright black, `whites` the normal and bright white, and
/// `hues` red · green · yellow · blue · magenta · cyan, the same six
/// answering both intensities.
fn ansi(blacks: [[u8; 3]; 2], whites: [[u8; 3]; 2], hues: [[u8; 3]; 6]) -> [Srgba; 16] {
    let h = hues.map(ink);
    [
        ink(blacks[0]),
        h[0],
        h[1],
        h[2],
        h[3],
        h[4],
        h[5],
        ink(whites[0]),
        ink(blacks[1]),
        h[0],
        h[1],
        h[2],
        h[3],
        h[4],
        h[5],
        ink(whites[1]),
    ]
}

/// An opaque color from the generated table's bytes.
fn ink(rgb: [u8; 3]) -> Srgba {
    faint(rgb, 1.0)
}

/// A color at `alpha` from the generated table's bytes.
fn faint(rgb: [u8; 3], alpha: f32) -> Srgba {
    Srgba::new(
        f32::from(rgb[0]) / 255.0,
        f32::from(rgb[1]) / 255.0,
        f32::from(rgb[2]) / 255.0,
        alpha,
    )
}

#[cfg(test)]
#[path = "lexicon.tests.rs"]
mod tests;
