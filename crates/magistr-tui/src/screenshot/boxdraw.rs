//! Box-drawing and block glyphs drawn from the cell, not from a font.
//!
//! A monospace face draws U+2500 in its advance width, and JetBrains
//! Mono advances 0.6 em — 9 px inside the [`Typeface`]'s 10 px cell. So
//! every `─` used to leave a column of background at the cell's right
//! edge and a panel ring read as a dashed line. Real
//! terminals — kitty, WezTerm, Ghostty — never ask the font for these:
//! they special-case the range and paint it to the cell's own pixel
//! edges, so a run joins because both neighbours reach the boundary.
//! This does the same, and [`super::rasterize`] asks here before it asks
//! the face.
//!
//! What is synthesized: the light and heavy lines, junctions, dashes and
//! stubs (U+2500–U+254B, U+254C–U+254F, U+2574–U+257F), the four arcs and
//! the diagonals (U+256D–U+2573), and the blocks, shades and quadrants
//! (U+2580–U+259F). The double-line family (U+2550–U+256C) is
//! deliberately **not**: a double junction has to nest two parallel
//! strokes correctly at every corner and tee, the cockpit draws none of
//! them, and [`glyph`] returning `None` falls straight through to the
//! embedded faces — so leaving them out costs nothing.

use super::glyph::Glyph;
use super::typeface::Typeface;

/// The cell's width in pixels, as an index and as a length.
const W: usize = Typeface::CELL_WIDTH as usize;
/// The cell's height in pixels.
const H: usize = Typeface::CELL_HEIGHT as usize;
/// The cell's width as a coordinate.
const WF: f32 = Typeface::CELL_WIDTH as f32;
/// The cell's height as a coordinate.
const HF: f32 = Typeface::CELL_HEIGHT as f32;

/// A light stroke's thickness.
const LIGHT: f32 = 1.0;
/// A heavy stroke's thickness — twice a light one, sharing its top or
/// left edge, so a light and a heavy line meet flush.
const HEAVY: f32 = 2.0;

/// The arms of every line glyph, one entry per code point from U+2500,
/// as `[up, right, down, left]` weights: 0 none, 1 light, 2 heavy. The
/// dashed slots (U+2504–U+250B) are `[0; 4]` — [`glyph`] routes those to
/// [`dashed`] before it reaches this table.
const LINES: [[u8; 4]; 0x4c] = [
    [0, 1, 0, 1], // ─
    [0, 2, 0, 2], // ━
    [1, 0, 1, 0], // │
    [2, 0, 2, 0], // ┃
    [0; 4],       // ┄ dashed
    [0; 4],       // ┅
    [0; 4],       // ┆
    [0; 4],       // ┇
    [0; 4],       // ┈
    [0; 4],       // ┉
    [0; 4],       // ┊
    [0; 4],       // ┋
    [0, 1, 1, 0], // ┌
    [0, 2, 1, 0], // ┍
    [0, 1, 2, 0], // ┎
    [0, 2, 2, 0], // ┏
    [0, 0, 1, 1], // ┐
    [0, 0, 1, 2], // ┑
    [0, 0, 2, 1], // ┒
    [0, 0, 2, 2], // ┓
    [1, 1, 0, 0], // └
    [1, 2, 0, 0], // ┕
    [2, 1, 0, 0], // ┖
    [2, 2, 0, 0], // ┗
    [1, 0, 0, 1], // ┘
    [1, 0, 0, 2], // ┙
    [2, 0, 0, 1], // ┚
    [2, 0, 0, 2], // ┛
    [1, 1, 1, 0], // ├
    [1, 2, 1, 0], // ┝
    [2, 1, 1, 0], // ┞
    [1, 1, 2, 0], // ┟
    [2, 1, 2, 0], // ┠
    [2, 2, 1, 0], // ┡
    [1, 2, 2, 0], // ┢
    [2, 2, 2, 0], // ┣
    [1, 0, 1, 1], // ┤
    [1, 0, 1, 2], // ┥
    [2, 0, 1, 1], // ┦
    [1, 0, 2, 1], // ┧
    [2, 0, 2, 1], // ┨
    [2, 0, 1, 2], // ┩
    [1, 0, 2, 2], // ┪
    [2, 0, 2, 2], // ┫
    [0, 1, 1, 1], // ┬
    [0, 1, 1, 2], // ┭
    [0, 2, 1, 1], // ┮
    [0, 2, 1, 2], // ┯
    [0, 1, 2, 1], // ┰
    [0, 1, 2, 2], // ┱
    [0, 2, 2, 1], // ┲
    [0, 2, 2, 2], // ┳
    [1, 1, 0, 1], // ┴
    [1, 1, 0, 2], // ┵
    [1, 2, 0, 1], // ┶
    [1, 2, 0, 2], // ┷
    [2, 1, 0, 1], // ┸
    [2, 1, 0, 2], // ┹
    [2, 2, 0, 1], // ┺
    [2, 2, 0, 2], // ┻
    [1, 1, 1, 1], // ┼
    [1, 1, 1, 2], // ┽
    [1, 2, 1, 1], // ┾
    [1, 2, 1, 2], // ┿
    [2, 1, 1, 1], // ╀
    [1, 1, 2, 1], // ╁
    [2, 1, 2, 1], // ╂
    [2, 1, 1, 2], // ╃
    [2, 2, 1, 1], // ╄
    [1, 1, 2, 2], // ╅
    [1, 2, 2, 1], // ╆
    [2, 2, 1, 2], // ╇
    [1, 2, 2, 2], // ╈
    [2, 1, 2, 2], // ╉
    [2, 2, 2, 1], // ╊
    [2, 2, 2, 2], // ╋
];

/// The one-armed stubs and the two mixed-weight lines, from U+2574.
const STUBS: [[u8; 4]; 12] = [
    [0, 0, 0, 1], // ╴
    [1, 0, 0, 0], // ╵
    [0, 1, 0, 0], // ╶
    [0, 0, 1, 0], // ╷
    [0, 0, 0, 2], // ╸
    [2, 0, 0, 0], // ╹
    [0, 2, 0, 0], // ╺
    [0, 0, 2, 0], // ╻
    [0, 2, 0, 1], // ╼
    [1, 0, 2, 0], // ╽
    [0, 1, 0, 2], // ╾
    [2, 0, 1, 0], // ╿
];

/// Which quarters each quadrant glyph inks, from U+2596, as bits
/// `upper-left | upper-right | lower-left | lower-right` (8, 4, 2, 1).
const QUADRANTS: [u8; 10] = [
    0b0010, // ▖
    0b0001, // ▗
    0b1000, // ▘
    0b1011, // ▙
    0b1001, // ▚
    0b1110, // ▛
    0b1101, // ▜
    0b0100, // ▝
    0b0110, // ▞
    0b0111, // ▟
];

/// The cell-sized coverage bitmap for `ch`, drawn from the cell's own
/// geometry — or `None` when `ch` is not one this module synthesizes, in
/// which case the caller falls back to the embedded faces.
///
/// The bitmap always covers the whole cell (`left`/`top` are 0), so
/// nothing about the face's metrics, advance or baseline can put a gap
/// at a cell boundary. Weight is not a parameter: a box-drawing glyph is
/// geometry, and a bold ─ is the same line — the heavy code points are
/// how the vocabulary asks for a thicker one.
#[must_use]
pub fn glyph(ch: char) -> Option<Glyph> {
    let cp = u32::from(ch);
    let mut ink = Ink::new();
    match cp {
        0x2504..=0x250b | 0x254c..=0x254f => dashed(&mut ink, cp),
        0x2500..=0x254b => arms(&mut ink, LINES[(cp - 0x2500) as usize]),
        0x256d..=0x2570 => corner(&mut ink, cp),
        0x2571..=0x2573 => diagonals(&mut ink, cp),
        0x2574..=0x257f => arms(&mut ink, STUBS[(cp - 0x2574) as usize]),
        0x2580..=0x259f => blocks(&mut ink, cp),
        _ => return None,
    }
    Some(ink.into_glyph())
}

/// A stroke's weight in pixels.
fn weight(code: u8) -> f32 {
    match code {
        1 => LIGHT,
        2 => HEAVY,
        _ => 0.0,
    }
}

/// Where a stroke of thickness `t` sits across an `extent`-long axis:
/// centred, then floored onto the pixel grid so a light line lands on
/// one whole pixel instead of straddling two at half coverage.
fn span(extent: f32, t: f32) -> (f32, f32) {
    let near = ((extent - t) / 2.0).floor();
    (near, near + t)
}

/// The four arms of a line glyph. Each arm runs from its cell edge to
/// the cell's midpoint, extended to cover the perpendicular stroke when
/// there is one — so a through-line spans the cell exactly and a corner
/// stops on the elbow. Each arm keeps its own weight, which is what a
/// mixed junction like `┍` is.
fn arms(ink: &mut Ink, a: [u8; 4]) {
    let (up, right, down, left) = (a[0], a[1], a[2], a[3]);
    let stile = weight(up).max(weight(down));
    let rule = weight(left).max(weight(right));
    let (vx0, vx1) = span(WF, stile);
    let (hy0, hy1) = span(HF, rule);
    let (midx, midy) = (WF / 2.0, HF / 2.0);
    // Without a perpendicular stroke to reach, an arm stops at the
    // middle — which is what makes ╴ and ╶ the same length.
    let (west, east) = if stile > 0.0 {
        (midx.max(vx1), midx.min(vx0))
    } else {
        (midx, midx)
    };
    let (north, south) = if rule > 0.0 {
        (midy.max(hy1), midy.min(hy0))
    } else {
        (midy, midy)
    };
    for (code, horizontal, from, to) in [
        (left, true, 0.0, west),
        (right, true, east, WF),
        (up, false, 0.0, north),
        (down, false, south, HF),
    ] {
        if code == 0 {
            continue;
        }
        if horizontal {
            let (y0, y1) = span(HF, weight(code));
            ink.rect(from, y0, to, y1);
        } else {
            let (x0, x1) = span(WF, weight(code));
            ink.rect(x0, from, x1, to);
        }
    }
}

/// A dashed line: `count` dashes across the cell, laid on a grid of
/// `2 * count - 1` equal parts so a dash and a gap are the same length.
fn dashed(ink: &mut Ink, cp: u32) {
    let (vertical, thick, count) = match cp {
        0x2504 => (false, false, 3),
        0x2505 => (false, true, 3),
        0x2506 => (true, false, 3),
        0x2507 => (true, true, 3),
        0x2508 => (false, false, 4),
        0x2509 => (false, true, 4),
        0x250a => (true, false, 4),
        0x250b => (true, true, 4),
        0x254c => (false, false, 2),
        0x254d => (false, true, 2),
        0x254e => (true, false, 2),
        _ => (true, true, 2),
    };
    let t = if thick { HEAVY } else { LIGHT };
    let extent = if vertical { HF } else { WF };
    let part = extent / (2.0 * count as f32 - 1.0);
    for i in (0..2 * count - 1).step_by(2) {
        let (from, to) = (i as f32 * part, (i as f32 + 1.0) * part);
        if vertical {
            let (x0, x1) = span(WF, t);
            ink.rect(x0, from, x1, to);
        } else {
            let (y0, y1) = span(HF, t);
            ink.rect(from, y0, to, y1);
        }
    }
}

/// One of the four arcs (U+256D–U+2570): the two straight arms plus a
/// quarter circle tangent to both.
///
/// The circle is **circular, not elliptical**. Fitting the arc to the
/// cell's aspect is kitty's issue #3409 — the radius stretches when the
/// cell is tall, and this cell is 10x20, the worst case for it.
///
/// The radius is the largest that still leaves a whole pixel of straight
/// run on each arm: the distance from the stroke's centre line to the
/// nearer cell edge, less one pixel. That is what makes the acceptance
/// hold — a corner meets its neighbouring `─` or `│` on solid pixels
/// rather than on the curve's tangent, where coverage is already falling.
fn corner(ink: &mut Ink, cp: u32) {
    let (down, right) = (cp == 0x256d || cp == 0x256e, cp == 0x256d || cp == 0x2570);
    let (vx0, vx1) = span(WF, LIGHT);
    let (hy0, hy1) = span(HF, LIGHT);
    let (sx, sy) = ((vx0 + vx1) / 2.0, (hy0 + hy1) / 2.0);
    let r = sx.min(WF - sx).min(sy).min(HF - sy) - 1.0;
    let (cx, cy) = (
        if right { sx + r } else { sx - r },
        if down { sy + r } else { sy - r },
    );
    if right {
        ink.rect(cx, hy0, WF, hy1);
    } else {
        ink.rect(0.0, hy0, cx, hy1);
    }
    if down {
        ink.rect(vx0, cy, vx1, HF);
    } else {
        ink.rect(vx0, 0.0, vx1, cy);
    }
    ink.arc(
        cx,
        cy,
        r,
        if right { -1.0 } else { 1.0 },
        if down { -1.0 } else { 1.0 },
    );
}

/// `╱`, `╲` and the `╳` that is both.
fn diagonals(ink: &mut Ink, cp: u32) {
    if cp != 0x2572 {
        ink.line(0.0, HF, WF, 0.0);
    }
    if cp != 0x2571 {
        ink.line(0.0, 0.0, WF, HF);
    }
}

/// The block elements: the halves and eighths, the three shades and the
/// quadrants. Every edge is a fraction of the cell rather than a rounded
/// pixel count, so an eighth of ten columns or of twenty rows lands where
/// it actually falls.
fn blocks(ink: &mut Ink, cp: u32) {
    match cp {
        0x2580 => ink.rect(0.0, 0.0, WF, HF / 2.0),
        0x2581..=0x2588 => {
            let eighths = (cp - 0x2580) as f32;
            ink.rect(0.0, HF * (8.0 - eighths) / 8.0, WF, HF);
        }
        0x2589..=0x258f => {
            let eighths = (0x2590 - cp) as f32;
            ink.rect(0.0, 0.0, WF * eighths / 8.0, HF);
        }
        0x2590 => ink.rect(WF / 2.0, 0.0, WF, HF),
        0x2591..=0x2593 => ink.wash((cp - 0x2590) as f32 / 4.0),
        0x2594 => ink.rect(0.0, 0.0, WF, HF / 8.0),
        0x2595 => ink.rect(WF * 7.0 / 8.0, 0.0, WF, HF),
        _ => {
            let quarters = QUADRANTS[(cp - 0x2596) as usize];
            let (midx, midy) = (WF / 2.0, HF / 2.0);
            for (bit, x0, y0) in [
                (0b1000, 0.0, 0.0),
                (0b0100, midx, 0.0),
                (0b0010, 0.0, midy),
                (0b0001, midx, midy),
            ] {
                if quarters & bit != 0 {
                    ink.rect(x0, y0, x0 + midx, y0 + midy);
                }
            }
        }
    }
}

/// The cell's coverage while it is being drawn, in `0.0..=1.0`. Shapes
/// combine by taking the greater of the two, so an arm overlapping a
/// junction's elbow — or an arc meeting its own arm — stays one solid
/// stroke instead of summing past full.
struct Ink(Vec<f32>);

impl Ink {
    fn new() -> Self {
        Self(vec![0.0; W * H])
    }

    /// Lay `v` over the pixel, keeping whichever is more covered.
    fn lay(&mut self, x: usize, y: usize, v: f32) {
        let v = v.clamp(0.0, 1.0);
        if let Some(p) = self.0.get_mut(y * W + x)
            && v > *p
        {
            *p = v;
        }
    }

    /// An axis-aligned rect in pixel coordinates. A pixel takes the area
    /// of its own overlap, so an edge at 2.5 covers half of pixel 2.
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        for y in 0..H {
            let dy = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).clamp(0.0, 1.0);
            if dy <= 0.0 {
                continue;
            }
            for x in 0..W {
                let dx = (x1.min(x as f32 + 1.0) - x0.max(x as f32)).clamp(0.0, 1.0);
                if dx > 0.0 {
                    self.lay(x, y, dx * dy);
                }
            }
        }
    }

    /// A light quarter circle of radius `r` about `(cx, cy)`, in the
    /// quadrant the two signs name: a pixel counts when `(centre - c)`
    /// has that sign on each axis, and its coverage falls off with its
    /// distance from the ideal curve.
    fn arc(&mut self, cx: f32, cy: f32, r: f32, sx: f32, sy: f32) {
        for y in 0..H {
            for x in 0..W {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                if (px - cx) * sx < 0.0 || (py - cy) * sy < 0.0 {
                    continue;
                }
                let d = (px - cx).hypot(py - cy);
                self.lay(x, y, LIGHT - (d - r).abs());
            }
        }
    }

    /// A light line through two points, anti-aliased by distance.
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = dx.hypot(dy);
        for y in 0..H {
            for x in 0..W {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let d = ((px - x0) * dy - (py - y0) * dx).abs() / len;
                self.lay(x, y, LIGHT - d);
            }
        }
    }

    /// Every pixel at the same partial coverage — what a shade is at this
    /// cell size. (A terminal that stipples them instead is drawing the
    /// same average with a texture no 10x20 cell can carry.)
    fn wash(&mut self, coverage: f32) {
        for y in 0..H {
            for x in 0..W {
                self.lay(x, y, coverage);
            }
        }
    }

    /// The finished bitmap, placed at the cell's own origin.
    fn into_glyph(self) -> Glyph {
        Glyph {
            left: 0,
            top: 0,
            width: W,
            height: H,
            coverage: self.0.iter().map(|v| (v * 255.0).round() as u8).collect(),
        }
    }
}

#[cfg(test)]
#[path = "boxdraw.tests.rs"]
mod tests;
