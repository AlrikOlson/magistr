use qs_gpu::color::Srgba;

/// CSS `color-mix(in oklab, a p%, b)` — the mixing space the volume's
/// control faces are specified in.
///
/// Every gradient stop and border of `.btn7` is written as a
/// `color-mix(in oklab, …)` over the palette's raw vars, so the atoms
/// derive those faces **live** from the same raw tokens rather than
/// hand-carrying the mixed results: a re-blessed palette re-mixes
/// itself. The math is Ottosson's reference implementation — the one
/// the CSS Color 4 spec adopts — with CSS's premultiplied-alpha
/// interpolation, which for the opaque colors the volume mixes reduces
/// to a straight lerp of L, a, b.
#[must_use]
pub fn mix(a: Srgba, b: Srgba, share_of_a: f32) -> Srgba {
    let p = share_of_a.clamp(0.0, 1.0);
    let (la, lb) = (to_oklab(a), to_oklab(b));
    let alpha = a.a * p + b.a * (1.0 - p);
    if alpha <= 0.0 {
        return Srgba::new(0.0, 0.0, 0.0, 0.0);
    }
    // CSS interpolates the coordinates premultiplied by alpha, then
    // divides the result back out.
    let lerp = |x: f32, y: f32| (x * a.a * p + y * b.a * (1.0 - p)) / alpha;
    from_oklab(
        [lerp(la[0], lb[0]), lerp(la[1], lb[1]), lerp(la[2], lb[2])],
        alpha,
    )
}

/// A color's oklab coordinates `[L, a, b]`.
fn to_oklab(c: Srgba) -> [f32; 3] {
    let (r, g, b) = (linear(c.r), linear(c.g), linear(c.b));
    let l = libm::cbrtf(0.412_221_46 * r + 0.536_332_54 * g + 0.051_445_995 * b);
    let m = libm::cbrtf(0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b);
    let s = libm::cbrtf(0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b);
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// The color at oklab `[L, a, b]` with `alpha`, clamped into sRGB.
fn from_oklab(lab: [f32; 3], alpha: f32) -> Srgba {
    let l = (lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2]).powi(3);
    let m = (lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2]).powi(3);
    let s = (lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2]).powi(3);
    Srgba::new(
        gamma(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_93 * s),
        gamma(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        gamma(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        alpha,
    )
}

/// One sRGB channel decoded to linear light.
fn linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        libm::powf((c + 0.055) / 1.055, 2.4)
    }
}

/// One linear channel encoded back to sRGB, clamped to the gamut.
fn gamma(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
#[path = "oklab.tests.rs"]
mod tests;
