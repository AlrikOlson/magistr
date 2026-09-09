//! A motion as a filmstrip: the frames it passes through, side by side.

use magistr_theme::Palette;
use tiny_skia::{Color as SkiaColor, Pixmap};

use super::frame::frame;
use super::profile::Profile;
use super::typeface::Typeface;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, update};
use magistr_cockpit::layout::LayoutError;
use magistr_cockpit::theme::MotionMode;

/// Cells of divider drawn between two frames of a strip.
const GUTTER_CELLS: u32 = 1;

/// `frames` successive frames of `model`, one animation step apart, laid
/// out left to right over a [`GUTTER_CELLS`]-wide divider — the
/// reviewable picture of a motion. Every animation the cockpit has is a
/// pure function of [`Model::ticks`](magistr_cockpit::cockpit::Model::ticks), and
/// the step between two panels is the same [`Msg::Tick`] the runtime
/// sends every `Model::TICK_PERIOD`, so a strip needs no clock and comes
/// out the same bytes on every machine.
///
/// `model` is left standing on the last panel's step. Under
/// [`MotionMode::Reduced`] nothing moves, so the strip is the single
/// still frame the surface draws — no tick is taken and the result is
/// byte-for-byte that surface's [`frame`]. A strip is never empty: zero
/// frames is one. `profile` is the terminal every panel is rasterized
/// as, so a motion on the pixel path is reviewed on the pixel path.
pub fn strip(
    model: &mut Model,
    clock: &FrameClock,
    frames: usize,
    profile: Profile,
) -> Result<Pixmap, LayoutError> {
    let wanted = if model.theme().motion() == MotionMode::Reduced {
        1
    } else {
        frames.max(1)
    };
    let mut panels = Vec::with_capacity(wanted);
    for step in 0..wanted {
        if step > 0 {
            update(model, Msg::Tick);
        }
        panels.push(frame(model, clock, profile)?);
    }
    Ok(join(&panels, &model.theme().palette()))
}

/// `panels` (all one size) side by side over the palette's divider. A
/// strip too large to allocate falls back to its first panel rather than
/// failing a review that has nothing to do with size.
fn join(panels: &[Pixmap], palette: &Palette) -> Pixmap {
    let Some(first) = panels.first() else {
        return Pixmap::new(1, 1).expect("a one-pixel pixmap always allocates");
    };
    let (width, height) = (first.width(), first.height());
    let gutter = GUTTER_CELLS * Typeface::CELL_WIDTH;
    let count = u32::try_from(panels.len()).unwrap_or(u32::MAX);
    let total = width
        .checked_mul(count)
        .and_then(|frames| frames.checked_add(gutter * (count - 1)))
        .unwrap_or(0);
    let Some(mut out) = Pixmap::new(total, height) else {
        return first.clone();
    };
    let divider = palette.overlay0;
    out.fill(SkiaColor::from_rgba8(divider.r, divider.g, divider.b, 255));
    for (index, panel) in panels.iter().enumerate() {
        let left = (width + gutter) * u32::try_from(index).unwrap_or(0);
        let span = width.min(panel.width()) as usize;
        for row in 0..height.min(panel.height()) {
            let src = (row * panel.width()) as usize;
            let dst = (row * total + left) as usize;
            out.pixels_mut()[dst..dst + span].copy_from_slice(&panel.pixels()[src..src + span]);
        }
    }
    out
}

#[cfg(test)]
#[path = "strip.tests.rs"]
mod tests;
