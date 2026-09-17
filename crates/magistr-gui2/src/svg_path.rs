use tiny_skia::{Path, PathBuilder};

/// The blessed volume's path data, as a `tiny-skia` [`Path`].
///
/// The icon set is copied out of the canonical volume's SVG sprite
/// **verbatim** ([`Icon`](crate::icon::Icon)), so the geometry in this
/// crate can be diffed against the page a human blessed rather than
/// being a re-drawing of it. That decision is what puts an SVG reader
/// here: `tiny-skia` has no arc verb and no rounded rectangle, so a hand
/// port would mean converting the sprite's four `A` icons and four
/// `rx` rectangles into cubics by eye — a translation nobody could
/// check against the source.
///
/// # The subset is the sprite's, not SVG's
///
/// Every `d` in the sprite uses exactly `M L H V A Z` and their relative
/// forms, so that is what is read. Anything else — a cubic, a quadratic,
/// a smooth continuation — answers `None` rather than being silently
/// skipped, because a glyph missing a stroke is a defect that must
/// surface as a rasterization failure and not as a picture that is
/// nearly right. Two details of the real grammar do appear here and are
/// handled: a command may carry **repeated** parameter sets (the pin's
/// `l-.7 3.6 2.2 2.4` is one `l` with two points), and a number may be
/// glued to the sign or dot before it (`h-9`, `-.7`).
///
/// An arc becomes at most four cubics, split at 90°, through the
/// endpoint→centre parameterization in the SVG specification's own
/// implementation notes (F.6.5/F.6.6) — including its correction of an
/// out-of-range radius, which the folder's corners rely on.
#[must_use]
pub fn svg_path(d: &str) -> Option<Path> {
    let mut builder = PathBuilder::new();
    let mut cursor = Cursor::new(d);
    // Where the pen is, and where the current subpath began — `Z`
    // returns to the latter, and a command after it draws from there.
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    let mut verb: Option<char> = None;
    loop {
        let next = cursor.verb();
        match next {
            Some(letter) => verb = Some(letter),
            // A repeated parameter set continues the previous verb; an
            // implicit `M` repeat is a `L`, which is the one place the
            // grammar renames the command it continues.
            None if cursor.at_end() => break,
            None => match verb {
                Some('M') => verb = Some('L'),
                Some('m') => verb = Some('l'),
                Some(_) => {}
                None => return None,
            },
        }
        let letter = verb?;
        let relative = letter.is_ascii_lowercase();
        let (ox, oy) = if relative { (x, y) } else { (0.0, 0.0) };
        match letter.to_ascii_uppercase() {
            'M' => {
                x = ox + cursor.number()?;
                y = oy + cursor.number()?;
                (sx, sy) = (x, y);
                builder.move_to(x, y);
            }
            'L' => {
                x = ox + cursor.number()?;
                y = oy + cursor.number()?;
                builder.line_to(x, y);
            }
            'H' => {
                x = ox + cursor.number()?;
                builder.line_to(x, y);
            }
            'V' => {
                y = oy + cursor.number()?;
                builder.line_to(x, y);
            }
            'A' => {
                let arc = Arc {
                    rx: cursor.number()?,
                    ry: cursor.number()?,
                    rotation: cursor.number()?,
                    large: cursor.number()? != 0.0,
                    sweep: cursor.number()? != 0.0,
                };
                let to = (ox + cursor.number()?, oy + cursor.number()?);
                arc.emit(&mut builder, (x, y), to);
                (x, y) = to;
            }
            'Z' => {
                builder.close();
                (x, y) = (sx, sy);
            }
            _ => return None,
        }
    }
    builder.finish()
}

/// A reader over one `d` string: verbs, numbers, and the separators
/// between them.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    const fn new(d: &'a str) -> Self {
        Self {
            bytes: d.as_bytes(),
            at: 0,
        }
    }

    /// Whitespace and commas are the same thing to SVG.
    fn skip(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b',')
        {
            self.at += 1;
        }
    }

    fn at_end(&mut self) -> bool {
        self.skip();
        self.at >= self.bytes.len()
    }

    /// The next command letter, if the next token is one.
    fn verb(&mut self) -> Option<char> {
        self.skip();
        let byte = *self.bytes.get(self.at)?;
        // `E`/`e` would be an exponent rather than a verb; the subset
        // has no command spelled with either, so rejecting them here
        // costs nothing and keeps `1e-3` readable as a number.
        if !byte.is_ascii_alphabetic() || byte.eq_ignore_ascii_case(&b'e') {
            return None;
        }
        self.at += 1;
        Some(char::from(byte))
    }

    /// The next number. A sign or a leading dot ends the previous one,
    /// which is what lets `l-.7 3.6` and `h-9` parse without separators.
    fn number(&mut self) -> Option<f32> {
        self.skip();
        let start = self.at;
        if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
            self.at += 1;
        }
        let mut seen_dot = false;
        while let Some(byte) = self.bytes.get(self.at) {
            match byte {
                b'0'..=b'9' => self.at += 1,
                b'.' if !seen_dot => {
                    seen_dot = true;
                    self.at += 1;
                }
                b'e' | b'E' => {
                    self.at += 1;
                    if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                        self.at += 1;
                    }
                }
                _ => break,
            }
        }
        std::str::from_utf8(self.bytes.get(start..self.at)?)
            .ok()?
            .parse()
            .ok()
    }
}

/// One elliptical arc, in the endpoint parameterization the `A` command
/// is written in.
struct Arc {
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
}

impl Arc {
    /// The largest sweep one cubic approximates well. 90° is the
    /// classic bound: the error of a cubic quarter-circle is under
    /// 0.03 % of the radius, which at a 24 px icon is far inside a
    /// pixel.
    const QUARTER: f32 = std::f32::consts::FRAC_PI_2;

    /// Append this arc from `from` to `to` as cubics.
    fn emit(&self, builder: &mut PathBuilder, from: (f32, f32), to: (f32, f32)) {
        let (rx, ry) = (self.rx.abs(), self.ry.abs());
        // A degenerate radius is a straight line, by the specification.
        if rx == 0.0 || ry == 0.0 || (from.0 == to.0 && from.1 == to.1) {
            builder.line_to(to.0, to.1);
            return;
        }
        let phi = self.rotation.to_radians();
        let (sin, cos) = libm::sincosf(phi);
        // F.6.5.1: the endpoints, halved and rotated into the ellipse's
        // own frame.
        let dx = (from.0 - to.0) / 2.0;
        let dy = (from.1 - to.1) / 2.0;
        let x1 = cos * dx + sin * dy;
        let y1 = -sin * dx + cos * dy;
        // F.6.6: an ellipse too small to reach both endpoints is scaled
        // up until it just does, rather than refused.
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        let scale = if lambda > 1.0 { lambda.sqrt() } else { 1.0 };
        let (rx, ry) = (rx * scale, ry * scale);
        let (local, centre) = self.centre((rx, ry), (x1, y1), from, to, (sin, cos));
        let start = angle_of((x1 - local.0) / rx, (y1 - local.1) / ry);
        let sweep = self.sweep_angle(start, angle_of((-x1 - local.0) / rx, (-y1 - local.1) / ry));
        // Split into equal steps of at most a quarter turn — equal so
        // the four cubics of a full circle are the same shape.
        let quarters = (sweep.abs() / Self::QUARTER).ceil().max(1.0);
        let step = sweep / quarters;
        let mut theta = start;
        let mut left = quarters;
        while left > 0.5 {
            cubic(builder, centre, (rx, ry), (sin, cos), theta, step);
            theta += step;
            left -= 1.0;
        }
        // The arithmetic lands within a rounding error of `to`; the
        // command says exactly where it ends, so say so.
        builder.line_to(to.0, to.1);
    }

    /// F.6.5.2–3: the ellipse's centre, in its own frame and in the
    /// user's.
    fn centre(
        &self,
        (rx, ry): (f32, f32),
        (x1, y1): (f32, f32),
        from: (f32, f32),
        to: (f32, f32),
        (sin, cos): (f32, f32),
    ) -> ((f32, f32), (f32, f32)) {
        let denominator = (rx * y1).mul_add(rx * y1, (ry * x1) * (ry * x1));
        let numerator = (rx * ry).mul_add(rx * ry, -denominator);
        let ratio = (numerator.max(0.0) / denominator).sqrt();
        let sign = if self.large == self.sweep { -1.0 } else { 1.0 };
        let cx1 = sign * ratio * (rx * y1) / ry;
        let cy1 = sign * ratio * -(ry * x1) / rx;
        let cx = (cos * cx1 - sin * cy1) + (from.0 + to.0) / 2.0;
        let cy = (sin * cx1 + cos * cy1) + (from.1 + to.1) / 2.0;
        ((cx1, cy1), (cx, cy))
    }

    /// F.6.5.6: the swept angle, put on the side of the circle the
    /// sweep flag asks for.
    fn sweep_angle(&self, start: f32, end: f32) -> f32 {
        let full = std::f32::consts::TAU;
        let mut delta = end - start;
        if self.sweep && delta < 0.0 {
            delta += full;
        } else if !self.sweep && delta > 0.0 {
            delta -= full;
        }
        delta
    }
}

/// The angle of a unit-frame vector, signed.
fn angle_of(x: f32, y: f32) -> f32 {
    libm::atan2f(y, x)
}

/// One cubic approximating `step` radians of the ellipse from `theta`.
fn cubic(
    builder: &mut PathBuilder,
    centre: (f32, f32),
    (rx, ry): (f32, f32),
    (sin, cos): (f32, f32),
    theta: f32,
    step: f32,
) {
    // The classic magic number: the control arm that makes a cubic
    // match a circular arc at both ends and at its midpoint.
    let alpha = (4.0 / 3.0) * libm::tanf(step / 4.0);
    let point = |t: f32| {
        let (st, ct) = libm::sincosf(t);
        let (ex, ey) = (rx * ct, ry * st);
        (
            centre.0 + cos * ex - sin * ey,
            centre.1 + sin * ex + cos * ey,
        )
    };
    let derivative = |t: f32| {
        let (st, ct) = libm::sincosf(t);
        let (ex, ey) = (-rx * st, ry * ct);
        (cos * ex - sin * ey, sin * ex + cos * ey)
    };
    let end = theta + step;
    let (p0, p1) = (point(theta), point(end));
    let (d0, d1) = (derivative(theta), derivative(end));
    builder.cubic_to(
        alpha.mul_add(d0.0, p0.0),
        alpha.mul_add(d0.1, p0.1),
        alpha.mul_add(-d1.0, p1.0),
        alpha.mul_add(-d1.1, p1.1),
        p1.0,
        p1.1,
    );
}

#[cfg(test)]
#[path = "svg_path.tests.rs"]
mod tests;
