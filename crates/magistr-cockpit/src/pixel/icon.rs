//! The vocabulary as vector glyphs, painted with transparency.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, PixmapMut, Stroke, StrokeDash,
    Transform,
};

/// One idea the cockpit marks — a fold, a status, a pin — as a glyph,
/// drawn centred and scaled to the shorter side of its pixmap on a
/// transparent background, in the ink the caller chose. The same
/// vocabulary the text sets spell ([`Icons::of`](crate::theme::Icons::of))
/// and the pen colours ([`Pen::ink`](crate::theme::Pen::ink)): one
/// name, one glyph in each medium.
///
/// Every glyph shares its stroke weight and its bounding square, so a
/// column of them reads as one set rather than as a font's leftovers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A folded row that can open.
    FoldClosed,
    /// An open row that can fold.
    FoldOpen,
    /// Succeeded.
    Done,
    /// Failed.
    Failed,
    /// Waiting to start.
    Pending,
    /// Next up: its dependencies are met.
    Ready,
    /// Cannot proceed.
    Blocked,
    /// Being worked right now; also "active", the thing in progress.
    InProgress,
    /// A pinned reasoning step.
    Pinned,
    /// The pin control on a step that is not pinned.
    Pin,
    /// A trace checkpoint.
    Checkpoint,
    /// A stakeholder signal.
    Signal,
    /// A roadmap proposal awaiting a decision.
    Proposal,
    /// Where the roadmap currently stands.
    Here,
    /// A list item's marker.
    Bullet,
    /// Something that needs the human's attention.
    Warning,
    /// Something that has stopped the work.
    Blocker,
    /// A program the reader stopped; a chunk held up by its dependencies.
    Stopped,
    /// A gate still deciding; a session that was cancelled.
    Waiting,
    /// A task passed over.
    Skipped,
    /// A task planned but not reached.
    Planned,
    /// Collapsing the rail.
    Collapse,
    /// A tool call in flight.
    Running,
    /// The spinner's frame — a three-quarter arc a quarter turn on per
    /// step, clockwise from the top.
    Spinner(u8),
    /// Undocking a pane into its own window.
    Undock,
    /// A file, and the read-only viewer showing one.
    File,
    /// An edit awaiting approval.
    Edit,
    /// The accelerator sheet.
    Keys,
}

/// The stroke's width as a fraction of the glyph's side.
const WEIGHT: f32 = 0.14;

impl Icon {
    /// Every glyph once — the spinner at its first frame — for the
    /// goldens that review the set.
    pub const ALL: [Self; 28] = [
        Self::FoldClosed,
        Self::FoldOpen,
        Self::Done,
        Self::Failed,
        Self::Pending,
        Self::Ready,
        Self::Blocked,
        Self::InProgress,
        Self::Pinned,
        Self::Pin,
        Self::Checkpoint,
        Self::Signal,
        Self::Proposal,
        Self::Here,
        Self::Bullet,
        Self::Warning,
        Self::Blocker,
        Self::Stopped,
        Self::Waiting,
        Self::Skipped,
        Self::Planned,
        Self::Collapse,
        Self::Running,
        Self::Spinner(0),
        Self::Undock,
        Self::File,
        Self::Edit,
        Self::Keys,
    ];

    /// The glyph's name — a golden's file stem.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FoldClosed => "fold-closed",
            Self::FoldOpen => "fold-open",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Blocked => "blocked",
            Self::InProgress => "in-progress",
            Self::Pinned => "pinned",
            Self::Pin => "pin",
            Self::Checkpoint => "checkpoint",
            Self::Signal => "signal",
            Self::Proposal => "proposal",
            Self::Here => "here",
            Self::Bullet => "bullet",
            Self::Warning => "warning",
            Self::Blocker => "blocker",
            Self::Stopped => "stopped",
            Self::Waiting => "waiting",
            Self::Skipped => "skipped",
            Self::Planned => "planned",
            Self::Collapse => "collapse",
            Self::Running => "running",
            Self::Spinner(_) => "spinner",
            Self::Undock => "undock",
            Self::File => "file",
            Self::Edit => "edit",
            Self::Keys => "keys",
        }
    }

    /// The glyph for a fold: open or shut.
    #[must_use]
    pub const fn fold(expanded: bool) -> Self {
        if expanded {
            Self::FoldOpen
        } else {
            Self::FoldClosed
        }
    }

    /// The glyph for a check's outcome.
    #[must_use]
    pub const fn check(passed: bool) -> Self {
        if passed { Self::Done } else { Self::Failed }
    }

    /// The glyph for a gate LED by its outcome (`None` while it runs).
    #[must_use]
    pub const fn gate(passed: Option<bool>) -> Self {
        match passed {
            None => Self::Waiting,
            Some(passed) => Self::check(passed),
        }
    }

    /// Draw the glyph into `pixmap` in `ink`, over a transparent ground.
    pub fn draw(self, pixmap: &mut PixmapMut<'_>, ink: Color) {
        pixmap.fill(Color::TRANSPARENT);
        let mut brush = Brush::new(pixmap, ink);
        self.trace(&mut brush);
    }

    /// The glyph's strokes and fills, one arm per glyph.
    #[allow(clippy::too_many_lines)] // the set, read top to bottom
    fn trace(self, brush: &mut Brush<'_, '_>) {
        match self {
            Self::FoldClosed => brush.stroke(&[(-0.14, -0.3), (0.16, 0.0), (-0.14, 0.3)], false),
            Self::FoldOpen => brush.stroke(&[(-0.3, -0.14), (0.0, 0.16), (0.3, -0.14)], false),
            Self::Collapse => brush.stroke(&[(0.14, -0.3), (-0.16, 0.0), (0.14, 0.3)], false),
            Self::Done => brush.stroke(&[(-0.34, 0.02), (-0.1, 0.26), (0.34, -0.24)], false),
            Self::Failed => {
                brush.stroke(&[(-0.26, -0.26), (0.26, 0.26)], false);
                brush.stroke(&[(0.26, -0.26), (-0.26, 0.26)], false);
            }
            Self::Pending => brush.ring(0.0, 0.0, 0.3),
            Self::Ready => brush.disc(0.0, 0.0, 0.3),
            Self::Running => {
                brush.ring(0.0, 0.0, 0.34);
                brush.disc(0.0, 0.0, 0.16);
            }
            Self::Planned => brush.disc(0.0, 0.0, 0.09),
            Self::Bullet => brush.disc(0.0, 0.0, 0.14),
            Self::Blocked => {
                brush.ring(0.0, 0.0, 0.32);
                brush.stroke(&[(-0.2, 0.2), (0.2, -0.2)], false);
            }
            Self::Blocker => {
                brush.ring(0.0, 0.0, 0.32);
                brush.stroke(&[(-0.14, -0.14), (0.14, 0.14)], false);
                brush.stroke(&[(0.14, -0.14), (-0.14, 0.14)], false);
            }
            Self::InProgress => brush.fill(&[(-0.24, -0.32), (0.32, 0.0), (-0.24, 0.32)]),
            Self::Here => brush.fill(&[(0.0, -0.34), (0.36, 0.3), (-0.36, 0.3)]),
            Self::Stopped => {
                brush.fill(&[(-0.26, -0.26), (0.26, -0.26), (0.26, 0.26), (-0.26, 0.26)]);
            }
            Self::Signal => brush.fill(&[(0.0, -0.38), (0.36, 0.0), (0.0, 0.38), (-0.36, 0.0)]),
            Self::Pinned => brush.fill(&star()),
            Self::Pin => brush.stroke(&star(), true),
            Self::Checkpoint => {
                brush.ring(0.0, 0.0, 0.34);
                brush.disc(0.0, 0.0, 0.12);
            }
            Self::Proposal => {
                brush.stroke(&[(-0.26, -0.38), (-0.26, 0.4)], false);
                brush.fill(&[(-0.26, -0.38), (0.32, -0.2), (-0.26, -0.02)]);
            }
            Self::Warning => {
                brush.stroke(&[(0.0, -0.36), (0.38, 0.3), (-0.38, 0.3)], true);
                brush.stroke(&[(0.0, -0.12), (0.0, 0.08)], false);
                brush.disc(0.0, 0.2, 0.05);
            }
            Self::Waiting => brush.dashed_ring(0.3, [1.6, 2.2], 0.0),
            Self::Spinner(step) => {
                let circumference = std::f32::consts::TAU * 0.32 * brush.side;
                let phase = circumference * f32::from(step % 4) / 4.0;
                brush.dashed_ring(0.32, [circumference * 0.72, circumference * 0.28], phase);
            }
            Self::Skipped => {
                brush.arc((-0.3, 0.16), (0.0, -0.5), (0.3, 0.16));
                brush.stroke(&[(0.1, 0.06), (0.3, 0.16), (0.34, -0.06)], false);
            }
            Self::Undock => {
                brush.stroke(
                    &[
                        (0.0, -0.3),
                        (-0.32, -0.3),
                        (-0.32, 0.32),
                        (0.3, 0.32),
                        (0.3, 0.0),
                    ],
                    false,
                );
                brush.stroke(&[(-0.04, 0.06), (0.32, -0.32)], false);
                brush.stroke(&[(0.08, -0.32), (0.32, -0.32), (0.32, -0.08)], false);
            }
            Self::File => {
                brush.stroke(
                    &[
                        (-0.26, -0.38),
                        (0.08, -0.38),
                        (0.26, -0.2),
                        (0.26, 0.38),
                        (-0.26, 0.38),
                    ],
                    true,
                );
                brush.stroke(&[(0.08, -0.38), (0.08, -0.2), (0.26, -0.2)], false);
            }
            Self::Edit => {
                brush.stroke(&[(-0.3, 0.3), (0.22, -0.22)], false);
                brush.stroke(&[(-0.3, 0.3), (-0.36, 0.36)], false);
                brush.stroke(&[(0.12, -0.32), (0.32, -0.12)], false);
            }
            Self::Keys => {
                brush.stroke(
                    &[(-0.38, -0.22), (0.38, -0.22), (0.38, 0.22), (-0.38, 0.22)],
                    true,
                );
                for x in [-0.2, 0.0, 0.2] {
                    brush.disc(x, -0.06, 0.04);
                }
                brush.stroke(&[(-0.18, 0.1), (0.18, 0.1)], false);
            }
        }
    }
}

/// A five-point star, tip up, in the unit square.
fn star() -> [(f32, f32); 10] {
    let mut points = [(0.0, 0.0); 10];
    for (index, point) in points.iter_mut().enumerate() {
        let radius = if index % 2 == 0 { 0.4 } else { 0.17 };
        let angle =
            (index as f32).mul_add(std::f32::consts::PI / 5.0, -std::f32::consts::FRAC_PI_2);
        *point = (radius * angle.cos(), radius * angle.sin());
    }
    points
}

/// The glyph's pen over one pixmap: unit coordinates mapped onto it,
/// one stroke weight, round caps and joins.
struct Brush<'a, 'p> {
    pixmap: &'a mut PixmapMut<'p>,
    side: f32,
    cx: f32,
    cy: f32,
    paint: Paint<'static>,
    stroke: Stroke,
}

impl<'a, 'p> Brush<'a, 'p> {
    fn new(pixmap: &'a mut PixmapMut<'p>, ink: Color) -> Self {
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let side = width.min(height);
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(ink);
        Self {
            pixmap,
            side,
            cx: width / 2.0,
            cy: height / 2.0,
            paint,
            stroke: Stroke {
                width: (side * WEIGHT).max(1.0),
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            },
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (x.mul_add(self.side, self.cx), y.mul_add(self.side, self.cy))
    }

    fn polyline(&self, points: &[(f32, f32)], closed: bool) -> Option<Path> {
        let mut path = PathBuilder::new();
        for (index, point) in points.iter().enumerate() {
            let (x, y) = self.at(*point);
            if index == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        if closed {
            path.close();
        }
        path.finish()
    }

    fn stroke_path(&mut self, path: &Path, stroke: &Stroke) {
        self.pixmap
            .stroke_path(path, &self.paint, stroke, Transform::identity(), None);
    }

    fn fill_path(&mut self, path: &Path) {
        self.pixmap.fill_path(
            path,
            &self.paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn stroke(&mut self, points: &[(f32, f32)], closed: bool) {
        if let Some(path) = self.polyline(points, closed) {
            let stroke = self.stroke.clone();
            self.stroke_path(&path, &stroke);
        }
    }

    fn fill(&mut self, points: &[(f32, f32)]) {
        if let Some(path) = self.polyline(points, true) {
            self.fill_path(&path);
        }
    }

    fn circle(&self, x: f32, y: f32, radius: f32) -> Option<Path> {
        let (cx, cy) = self.at((x, y));
        PathBuilder::from_circle(cx, cy, radius * self.side)
    }

    fn ring(&mut self, x: f32, y: f32, radius: f32) {
        if let Some(circle) = self.circle(x, y, radius) {
            let stroke = self.stroke.clone();
            self.stroke_path(&circle, &stroke);
        }
    }

    fn disc(&mut self, x: f32, y: f32, radius: f32) {
        if let Some(circle) = self.circle(x, y, radius) {
            self.fill_path(&circle);
        }
    }

    /// A centred circle stroked with `dash` (on, off — in pixels) from
    /// `phase` pixels along it.
    fn dashed_ring(&mut self, radius: f32, dash: [f32; 2], phase: f32) {
        let Some(circle) = self.circle(0.0, 0.0, radius) else {
            return;
        };
        let mut stroke = self.stroke.clone();
        stroke.dash = StrokeDash::new(dash.to_vec(), phase);
        self.stroke_path(&circle, &stroke);
    }

    /// A quadratic arc from `from` to `to` bent toward `control`.
    fn arc(&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32)) {
        let mut path = PathBuilder::new();
        let (x, y) = self.at(from);
        path.move_to(x, y);
        let (kx, ky) = self.at(control);
        let (x, y) = self.at(to);
        path.quad_to(kx, ky, x, y);
        if let Some(path) = path.finish() {
            let stroke = self.stroke.clone();
            self.stroke_path(&path, &stroke);
        }
    }
}

#[cfg(test)]
#[path = "icon.tests.rs"]
mod tests;
