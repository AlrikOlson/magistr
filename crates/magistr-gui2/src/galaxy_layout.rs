use magistr_cockpit::cockpit::GalaxyProject;
use magistr_cockpit::layout::PxRect;

use crate::cluster::Cluster;
use crate::placed_cluster::PlacedCluster;

/// Where the Galaxy's clusters sit on its field, in the field's own logical pixels —
/// one pure function the composition (which pins the labels) and the drawing (which
/// puts down the stars) both call with the same field, so the two cannot drift by a
/// pixel.
///
/// # Derived, not tabulated
///
/// The volume places its three clusters by hand. This places *N* clusters by rule: each
/// owns a sector of the field around a centre a little above the middle (the foot's
/// chrome is below), the first sector opening to the left and the rest sweeping
/// clockwise, each cluster as far out as its density asks — a denser cluster needs more
/// room, so it orbits further from the centre, which is also how the volume's three sit
/// (seven stars at the far ring, three near the middle). So three projects read left /
/// upper-right / lower-right as the volume's do, and a fourth finds its own sector
/// rather than a spare slot. The zoom scales the radii and the clusters together:
/// scrolling in spreads the constellations and grows them, the way a map zooms.
///
/// A cluster's *shape* is [`Cluster`]'s — the project's name and numbers — and its
/// label anchors under its own stars' bounds, so a dense cluster and a sparse one each
/// carry their name where the capture carries it: a little left of the first star,
/// below the last.
pub struct GalaxyLayout;

impl GalaxyLayout {
    /// A cluster's scale at zoom 1, in logical px per unit on each axis
    /// — the unit box [`Cluster`] answers in is two wide and one tall,
    /// so this spreads a cluster to the volume's own measure (magistr's
    /// stars span about 180 by 90).
    pub const RX: f32 = 90.0;
    pub const RY: f32 = 90.0;
    /// How far out a cluster orbits per star it holds, as a share of
    /// the field's smaller side — the volume's three, measured: about
    /// forty pixels a star on a 553-pixel field.
    const RING_PER_STAR: f32 = 0.072;
    /// Where the first sector opens: a little below due left, the way
    /// the volume's first cluster sits.
    const FIRST_SECTOR: f32 = 195.0_f32.to_radians();
    /// The orbit is an ellipse, four fifths as tall as it is wide: the
    /// field is wider than tall, and a round orbit would push the top
    /// and bottom clusters into the header and the foot.
    const ORBIT_SQUASH: f32 = 0.8;
    /// Where the ring's centre sits down the field — a little above the
    /// middle, so the clusters clear the foot's button.
    const CENTRE_Y: f32 = 0.47;
    /// The label's offset from the cluster's stars: a little left of
    /// the leftmost star, below the lowest (the volume's own margins,
    /// averaged over its three).
    const LABEL_DX: f32 = -8.0;
    const LABEL_DY: f32 = 26.0;
    /// The label block's measure, for the click box: the project name,
    /// status, and Open project control.
    const LABEL_W: f32 = 150.0;
    const LABEL_H: f32 = 80.0;
    /// The air a click box keeps around the stars.
    const REACH: f32 = 12.0;

    /// Every project placed on a `field` of that size at `zoom`, in the
    /// order the projects were given — the order their ids count.
    #[must_use]
    pub fn place(projects: &[GalaxyProject], field: (f32, f32), zoom: f32) -> Vec<PlacedCluster> {
        let (width, height) = field;
        let count = projects.len();
        let centre = (width * 0.5, height * Self::CENTRE_Y);
        let sector = std::f32::consts::TAU / count.max(1) as f32;
        projects
            .iter()
            .enumerate()
            .map(|(index, project)| {
                let cluster = Cluster::of(project);
                let ring = if count > 1 {
                    width.min(height) * Self::RING_PER_STAR * cluster.stars().len() as f32 * zoom
                } else {
                    0.0
                };
                let angle = (index as f32).mul_add(sector, Self::FIRST_SECTOR);
                let anchor = (
                    ring.mul_add(libm::cosf(angle), centre.0),
                    (ring * Self::ORBIT_SQUASH).mul_add(libm::sinf(angle), centre.1),
                );
                Self::placed(&cluster, anchor, zoom)
            })
            .collect()
    }

    /// `cluster` scaled to the field and centred on `anchor`.
    fn placed(cluster: &Cluster, anchor: (f32, f32), zoom: f32) -> PlacedCluster {
        let stars: Vec<(f32, f32)> = cluster
            .stars()
            .iter()
            .map(|&(x, y)| {
                (
                    (Self::RX * zoom).mul_add(x, anchor.0),
                    (Self::RY * zoom).mul_add(y, anchor.1),
                )
            })
            .collect();
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in &stars {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let label = (x0 + Self::LABEL_DX, y1 + Self::LABEL_DY);
        let left = (x0 - Self::REACH).min(label.0);
        let top = y0 - Self::REACH;
        let right = (x1 + Self::REACH).max(label.0 + Self::LABEL_W);
        let bottom = label.1 + Self::LABEL_H;
        PlacedCluster {
            stars,
            states: cluster.states().to_vec(),
            edges: cluster.edges(),
            label,
            bounds: PxRect::new(left, top, right - left, bottom - top),
        }
    }
}

#[cfg(test)]
#[path = "galaxy_layout.tests.rs"]
mod tests;
