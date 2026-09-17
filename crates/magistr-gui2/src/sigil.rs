use crate::icon::Icon;
use crate::seed::Seed;
use crate::shape::Shape;

/// A project's sigil: the mini-constellation mark derived from its
/// name.
///
/// `docs/architecture/desktop.md#icons`: "a generated mini-constellation mark per
/// project (derived from the name), worn in the galaxy, the breadcrumb
/// identity, and reports." Three or four stars on the icon set's own
/// 16-unit grid, joined by a chain of segments that stop short of each
/// star so the dot reads as a body and not as a fat line cap.
///
/// # Derived, not tabulated
///
/// The blessed volume draws four sigils by hand — `sig-magistr`,
/// `sig-tidewatch`, `sig-tas`, `sig-qs` — and those four are what the
/// *grammar* here is read off: 3–4 stars at radius 1.2, inside a
/// margin, joined in a chain, each segment inset from the star centres
/// it runs between. What is deliberately **not** copied is their
/// positions. A table of four names is a lookup, and the design asks
/// for a derivation: a project magistr has never heard of must get a
/// mark, and it must be the same mark on every machine. So
/// [`Sigil::of`] answers for any string, and the volume's four names
/// are simply the four the specimen sheet shows.
///
/// # Determinism is across builds, not just within one
///
/// The stream is a [`Seed`] — FNV-1a and SplitMix64 written out rather
/// than `DefaultHasher`, whose output is explicitly not stable across
/// Rust releases — a sigil that changed shape when the toolchain moved
/// would be a different project's mark on the same project. The
/// Galaxy's clusters draw from the same seed, so a project's two marks
/// come from one name.
///
/// # The stars cannot collide
///
/// Each star owns an angular sector of the mark, and its jitter is
/// under half a sector, so two stars are separated **by
/// construction** rather than by a rejection loop that might not
/// terminate. That is also what keeps the derivation a pure function of
/// the name with no iteration count to tune.
#[derive(Clone, Debug, PartialEq)]
pub struct Sigil {
    stars: Vec<(f32, f32)>,
}

impl Sigil {
    /// A star's radius, in grid units — the volume's own `r="1.2"`.
    pub const STAR: f32 = 1.2;

    /// The four projects the blessed volume draws a sigil for, and the
    /// four the specimen sheet shows. They are a *demonstration* of the
    /// derivation, not a table it reads: `Sigil::of` answers for any
    /// name, and these four have no privilege in it.
    pub const VOLUME_PROJECTS: [&'static str; 4] =
        ["magistr", "tidewatch", "think-and-ship", "quicksilver"];

    /// How far a joining segment stops short of a star's centre. The
    /// volume's four inset theirs by 0.5–0.8 units; a shade over the
    /// star's own radius keeps the dot clear of the line's round cap.
    const GAP: f32 = 1.6;

    /// The ring the stars are placed on, in grid units from the
    /// centre. Bounded so that a star's own radius stays inside the
    /// grid's margin.
    const NEAR: f32 = 3.0;
    const FAR: f32 = 5.4;

    /// How far a star may wander inside its own sector, as a share of
    /// it. Under a half, so two neighbours always keep at least half a
    /// sector between them.
    const JITTER: f32 = 0.22;

    /// The sigil for `name`.
    ///
    /// The same string always answers the same mark, on any machine and
    /// any toolchain; leading and trailing space and letter case do not
    /// change it, because `Magistr ` and `magistr` are the same project
    /// to a human reading a breadcrumb.
    #[must_use]
    pub fn of(name: &str) -> Self {
        let mut bits = Seed::of(name);
        // Three or four, the range the volume's own four span.
        let count = 3 + usize::from(bits.byte() % 2 == 1);
        let centre = Icon::GRID / 2.0;
        let sector = std::f32::consts::TAU / count as f32;
        let mut stars: Vec<(f32, f32)> = (0..count)
            .map(|index| {
                let along = f32::from(bits.byte()) / 255.0;
                let out = f32::from(bits.byte()) / 255.0;
                let angle = (index as f32).mul_add(sector, (along - 0.5) * Self::JITTER * sector);
                let radius = (Self::FAR - Self::NEAR).mul_add(out, Self::NEAR);
                (
                    radius.mul_add(libm::cosf(angle), centre),
                    radius.mul_add(libm::sinf(angle), centre),
                )
            })
            .collect();
        // The *order* the chain visits them in is derived too, and it
        // is what actually makes two projects look unalike. Positions
        // alone do not: every star owns a sector, so a chain that
        // always ran through them in sector order would draw the same
        // zigzag for every name and differ only by a nudge — which is
        // exactly what the first specimen sheet showed. Shuffling the
        // visiting order leaves the sectors (and therefore the
        // separation guarantee) untouched while giving a four-star mark
        // twelve distinct shapes instead of one.
        bits.shuffle(&mut stars);
        Self { stars }
    }

    /// Where the mark's stars sit on the 16-unit grid.
    #[must_use]
    pub fn stars(&self) -> &[(f32, f32)] {
        &self.stars
    }

    /// The mark's elements: the chain, then the stars over it.
    ///
    /// The chain is drawn first so a star's fill sits on top of the
    /// segment that reaches it, which is how the volume's four are
    /// layered.
    #[must_use]
    pub fn shapes(&self) -> Vec<Shape> {
        let mut shapes: Vec<Shape> = self
            .stars
            .windows(2)
            .filter_map(|pair| segment(pair[0], pair[1]))
            .collect();
        shapes.extend(self.stars.iter().map(|&(cx, cy)| Shape::Star {
            cx,
            cy,
            r: Self::STAR,
        }));
        shapes
    }
}

/// The segment joining two stars, inset from both centres.
///
/// `None` when the two are closer together than the two insets, which
/// the sector rule makes unreachable — but a chain with a zero-length
/// segment in it would draw a round cap where there should be nothing,
/// so the case is answered rather than assumed away.
fn segment(from: (f32, f32), to: (f32, f32)) -> Option<Shape> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = libm::hypotf(dx, dy);
    if length <= Sigil::GAP * 2.0 {
        return None;
    }
    let (ux, uy) = (dx / length, dy / length);
    Some(Shape::Segment {
        from: (
            Sigil::GAP.mul_add(ux, from.0),
            Sigil::GAP.mul_add(uy, from.1),
        ),
        to: (Sigil::GAP.mul_add(-ux, to.0), Sigil::GAP.mul_add(-uy, to.1)),
    })
}

#[cfg(test)]
#[path = "sigil.tests.rs"]
mod tests;
