//! Color math utilities — luminance, contrast, darken, lighten, lerp.
//!
//! Framework-agnostic implementations operating on `ThemeColor`.

use crate::color::ThemeColor;

/// Compute the relative luminance of an sRGB color per WCAG 2.1.
///
/// Returns a value in `[0.0, 1.0]` where 0.0 is black and 1.0 is white.
pub fn relative_luminance(c: ThemeColor) -> f64 {
    let r = linearize(c.r);
    let g = linearize(c.g);
    let b = linearize(c.b);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Linearize an sRGB channel value (0–255) to linear light.
fn linearize(channel: u8) -> f64 {
    let c = channel as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Compute the WCAG 2.1 contrast ratio between two colors.
///
/// Returns a value in `[1.0, 21.0]`. WCAG AA requires >= 4.5 for normal text,
/// >= 3.0 for large text. WCAG AAA requires >= 7.0 for normal text.
pub fn contrast_ratio(a: ThemeColor, b: ThemeColor) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// A contrast violation found during palette validation.
#[derive(Debug, Clone)]
pub struct ContrastViolation {
    /// Description of the color pair (e.g. "text on base").
    pub pair: String,
    /// The measured contrast ratio.
    pub ratio: f64,
    /// The required minimum ratio.
    pub required: f64,
}

/// Check whether a foreground/background pair meets a minimum contrast ratio.
///
/// Returns `Some(ContrastViolation)` if the pair fails, `None` if it passes.
pub fn validate_contrast(
    fg: ThemeColor,
    bg: ThemeColor,
    min_ratio: f64,
    pair_name: &str,
) -> Option<ContrastViolation> {
    let ratio = contrast_ratio(fg, bg);
    if ratio < min_ratio {
        Some(ContrastViolation {
            pair: pair_name.to_string(),
            ratio,
            required: min_ratio,
        })
    } else {
        None
    }
}

/// `fg` moved away from `bg` until the pair clears `min_ratio`.
///
/// A pair that already passes is returned untouched. Otherwise the
/// colour walks in fiftieths toward whichever extreme — white or black —
/// contrasts better with `bg`, and the first shade that clears the floor
/// wins; the extreme itself is the answer when none does. Since the
/// better extreme always clears 4.5:1 against any background, a 3:1 or
/// 4.5:1 floor is always reachable.
///
/// This is how a surface guarantees legibility it cannot get from the
/// palette: an accent that fails on its own background (see
/// [`crate::Palette::validate`]) is nudged rather than redesigned.
#[must_use]
pub fn ensure_contrast(fg: ThemeColor, bg: ThemeColor, min_ratio: f64) -> ThemeColor {
    /// Shades tried between the colour and the extreme.
    const STEPS: u16 = 50;
    if contrast_ratio(fg, bg) >= min_ratio {
        return fg;
    }
    let white = ThemeColor::new(255, 255, 255);
    let black = ThemeColor::new(0, 0, 0);
    let target = if contrast_ratio(white, bg) >= contrast_ratio(black, bg) {
        white
    } else {
        black
    };
    for step in 1..=STEPS {
        let candidate = lerp(fg, target, f64::from(step) / f64::from(STEPS));
        if contrast_ratio(candidate, bg) >= min_ratio {
            return candidate;
        }
    }
    target
}

/// Darken a color by `amount` (0.0 = unchanged, 1.0 = black).
pub fn darken(c: ThemeColor, amount: f64) -> ThemeColor {
    let factor = 1.0 - amount.clamp(0.0, 1.0);
    ThemeColor::new(
        (c.r as f64 * factor).round() as u8,
        (c.g as f64 * factor).round() as u8,
        (c.b as f64 * factor).round() as u8,
    )
}

/// Lighten a color by `amount` (0.0 = unchanged, 1.0 = white).
///
/// Each channel moves `amount` fraction of the way to 255.
pub fn lighten(c: ThemeColor, amount: f64) -> ThemeColor {
    let t = amount.clamp(0.0, 1.0);
    ThemeColor::new(
        (c.r as f64 + (255.0 - c.r as f64) * t).round() as u8,
        (c.g as f64 + (255.0 - c.g as f64) * t).round() as u8,
        (c.b as f64 + (255.0 - c.b as f64) * t).round() as u8,
    )
}

/// Linear interpolation between two colors.
///
/// `t` is clamped to `[0.0, 1.0]`. At `t=0.0` returns `a`, at `t=1.0` returns `b`.
pub fn lerp(a: ThemeColor, b: ThemeColor, t: f64) -> ThemeColor {
    let t = t.clamp(0.0, 1.0);
    ThemeColor::new(
        lerp_u8(a.r, b.r, t),
        lerp_u8(a.g, b.g, t),
        lerp_u8(a.b, b.b, t),
    )
}

/// Linear interpolation between two `u8` values.
fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).round() as u8
}

#[cfg(test)]
#[path = "math.tests.rs"]
mod tests;
