use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::lexicon::Lexicon;

/// The theme cards' live media: the sky, the chart and the OS split, each the volume's
/// own inline SVG on view 10's 132×74 drawing, scaled to whatever box the card asks
/// for. The literals are the volume's — media content, not palette: a preview shows the
/// *other* medium too, so no lexicon pen can stand in for its inks.
pub struct ThemePreview;

impl ThemePreview {
    /// The drawing every coordinate below is on (`.tp` in view 10).
    pub const DRAWING: (f32, f32) = (132.0, 74.0);
    /// The preview box's corner (`.tp { border-radius:9px }`) — the
    /// split's strips follow it, since the painter does not clip.
    const RADIUS: f32 = 9.0;
    /// The night ground (`#14141f`) and the day paper (`#f1f0f5`).
    const NIGHT: Srgba = Srgba::new(20.0 / 255.0, 20.0 / 255.0, 31.0 / 255.0, 1.0);
    const PAPER: Srgba = Srgba::new(241.0 / 255.0, 240.0 / 255.0, 245.0 / 255.0, 1.0);
    /// The chart's graticule (`rgba(139,138,160,.25)` every 14px).
    const GRATICULE: Srgba = Srgba::new(139.0 / 255.0, 138.0 / 255.0, 160.0 / 255.0, 0.25);
    const GRATICULE_STEP: f32 = 14.0;
    /// The constellation's edges: `#26263a` at 1px on the sky, `#d9d7e4`
    /// at 1.2px on the chart.
    const NIGHT_EDGE: Srgba = Srgba::new(38.0 / 255.0, 38.0 / 255.0, 58.0 / 255.0, 1.0);
    const DAY_EDGE: Srgba = Srgba::new(217.0 / 255.0, 215.0 / 255.0, 228.0 / 255.0, 1.0);
    /// The two edges the drawing joins: (30,50)→(58,30)→(88,44).
    const EDGES: [((f32, f32), (f32, f32)); 2] =
        [((30.0, 50.0), (58.0, 30.0)), ((58.0, 30.0), (88.0, 44.0))];
    /// The sky's stars: `#a3e0a0` and `#c7a4f5`.
    const NIGHT_GREEN: Srgba = Srgba::new(163.0 / 255.0, 224.0 / 255.0, 160.0 / 255.0, 1.0);
    const NIGHT_MAUVE: Srgba = Srgba::new(199.0 / 255.0, 164.0 / 255.0, 245.0 / 255.0, 1.0);
    /// The chart's marks: `#2f8f2a` and `#7d3fd0`.
    const DAY_GREEN: Srgba = Srgba::new(47.0 / 255.0, 143.0 / 255.0, 42.0 / 255.0, 1.0);
    const DAY_MAUVE: Srgba = Srgba::new(125.0 / 255.0, 63.0 / 255.0, 208.0 / 255.0, 1.0);
    /// The split's slant — `linear-gradient(105deg, night 49.8%, paper
    /// 50.2%)`: the seam through the centre, tilted 15° off vertical.
    const SPLIT_DEG: f32 = 105.0;

    /// The sky (night): the deep ground with three stars.
    #[must_use]
    pub fn sky(width: f32, height: f32) -> Element {
        let (sx, sy) = scale(width, height);
        edges(
            Element::surface().fill(Self::NIGHT),
            sx,
            sy,
            Self::NIGHT_EDGE,
        )
        .child(Self::dot(30.0 * sx, 50.0 * sy, 2.5 * sx, Self::NIGHT_GREEN))
        .child(Self::dot(58.0 * sx, 30.0 * sy, 2.5 * sx, Self::NIGHT_GREEN))
        .child(Self::dot(88.0 * sx, 44.0 * sy, 3.5 * sx, Self::NIGHT_MAUVE))
    }

    /// The chart (day): graticule paper with the same three marks, the
    /// mauve one ringed the way ink marks a star.
    #[must_use]
    pub fn chart(width: f32, height: f32) -> Element {
        let (sx, sy) = scale(width, height);
        let mut paper = Element::surface().fill(Self::PAPER);
        let line = Lexicon::composite(Self::PAPER, Self::GRATICULE);
        let mut x = Self::GRATICULE_STEP * sx;
        while x < width {
            paper = paper.child(
                Element::surface()
                    .pin_left(x.round())
                    .pin_top(0.0)
                    .pin_bottom(0.0)
                    .width(1.0)
                    .fill(line),
            );
            x += Self::GRATICULE_STEP * sx;
        }
        let mut y = Self::GRATICULE_STEP * sy;
        while y < height {
            paper = paper.child(
                Element::surface()
                    .pin_left(0.0)
                    .pin_right(0.0)
                    .pin_top(y.round())
                    .height(1.0)
                    .fill(line),
            );
            y += Self::GRATICULE_STEP * sy;
        }
        let ring = 6.0 * sx;
        edges(paper, sx, sy, Self::DAY_EDGE)
            .child(Self::dot(30.0 * sx, 50.0 * sy, 2.5 * sx, Self::DAY_GREEN))
            .child(Self::dot(58.0 * sx, 30.0 * sy, 2.5 * sx, Self::DAY_GREEN))
            .child(Self::dot(88.0 * sx, 44.0 * sy, 2.5 * sx, Self::DAY_MAUVE))
            .child(
                Element::surface()
                    .width(ring * 2.0)
                    .height(ring * 2.0)
                    .radius(ring)
                    .stroke(Self::DAY_MAUVE, 1.0)
                    .pin_left(88.0 * sx - ring)
                    .pin_top(44.0 * sy - ring),
            )
    }

    /// Follow OS: night on the left, paper on the right, split on the
    /// volume's slant — drawn as one-pixel strips, because the painter
    /// has no angled gradient and does not clip: each strip starts on
    /// the seam and ends inside the box's rounded corner.
    #[must_use]
    pub fn split(width: f32, height: f32) -> Element {
        let (sx, sy) = scale(width, height);
        let mut card = Element::surface().fill(Self::NIGHT);
        let (dx, dy) = Self::SPLIT_DEG.to_radians().sin_cos();
        // CSS's gradient line: `(w·|sin θ| + h·|cos θ|)` long through
        // the centre, direction (sin θ, −cos θ); the 50% stop is the
        // line through the centre perpendicular to it.
        let (cx, cy) = (width / 2.0, height / 2.0);
        let mut y = 0.0;
        while y < height {
            let yc = y + 0.5;
            // (x − cx)·sin θ + (y − cy)·(−cos θ) = 0 ⇒ the seam's x.
            let seam = cx + (yc - cy) * dy / dx;
            let seam = seam.round().clamp(0.0, width);
            let inset = corner_inset(yc, height, Self::RADIUS);
            let right = (width - inset).round();
            if right > seam {
                card = card.child(
                    Element::surface()
                        .pin_left(seam)
                        .pin_top(y)
                        .width(right - seam)
                        .height(1.0)
                        .fill(Self::PAPER),
                );
            }
            y += 1.0;
        }
        card.child(Self::dot(34.0 * sx, 38.0 * sy, 2.5 * sx, Self::NIGHT_GREEN))
            .child(Self::dot(98.0 * sx, 38.0 * sy, 2.5 * sx, Self::DAY_GREEN))
    }

    /// One star or mark: a disc of radius `r` centred at (`cx`, `cy`),
    /// the SVG circle's own addressing.
    #[must_use]
    pub fn dot(cx: f32, cy: f32, r: f32, ink: Srgba) -> Element {
        Element::surface()
            .width(r * 2.0)
            .height(r * 2.0)
            .radius(r)
            .fill(ink)
            .pin_left(cx - r)
            .pin_top(cy - r)
    }
}

/// The constellation's two edges drawn onto `ground` as one-pixel
/// dots along the longer axis — the painter has no line primitive,
/// and a 1px SVG stroke is a run of pixels.
fn edges(mut ground: Element, sx: f32, sy: f32, ink: Srgba) -> Element {
    for ((x0, y0), (x1, y1)) in ThemePreview::EDGES {
        let (x0, y0, x1, y1) = (x0 * sx, y0 * sy, x1 * sx, y1 * sy);
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).round().max(1.0);
        let mut i = 0.0;
        while i <= steps {
            let t = i / steps;
            let x = (x0 + (x1 - x0) * t).round();
            let y = (y0 + (y1 - y0) * t).round();
            ground = ground.child(
                Element::surface()
                    .pin_left(x)
                    .pin_top(y)
                    .width(1.0)
                    .height(1.0)
                    .fill(ink),
            );
            i += 1.0;
        }
    }
    ground
}

/// The box's scale off the drawing.
fn scale(width: f32, height: f32) -> (f32, f32) {
    (
        width / ThemePreview::DRAWING.0,
        height / ThemePreview::DRAWING.1,
    )
}

/// How far a strip centred on `yc` stops short of the right edge to
/// stay inside a corner of radius `r` on a box `height` tall.
fn corner_inset(yc: f32, height: f32, r: f32) -> f32 {
    let dy = if yc < r {
        r - yc
    } else if yc > height - r {
        yc - (height - r)
    } else {
        return 0.0;
    };
    r - (r * r - dy * dy).max(0.0).sqrt()
}

#[cfg(test)]
#[path = "theme_preview.tests.rs"]
mod tests;
