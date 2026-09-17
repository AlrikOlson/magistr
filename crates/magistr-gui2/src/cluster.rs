use magistr_cockpit::cockpit::{GalaxyProject, StarState};

use crate::seed::Seed;

/// A project's cluster on the Galaxy: a mini-constellation derived from the project's
/// name and its numbers.
///
/// # What the numbers decide and what the name decides
///
/// The **count** is the project's history: three stars for a project with nothing
/// shipped yet, one more for every sixty-four chunks shipped, capped so a long history
/// reads as a dense cluster rather than a field of its own. A live session adds a
/// working star and a pending question an asking star, both at the chain's tail, so the
/// eye finds the news at the end of the line. The **shape** — where the stars sit and
/// the order the chain visits them — is the name's, through the same [`Seed`] the
/// project's sigil draws from: the same project on two machines is the same cluster.
///
/// The positions are unit coordinates in a box two wide and one tall, `-1..=1` by
/// `-0.5..=0.5`; the layout scales them to the field.
///
/// # The stars cannot collide
///
/// Each star owns an angular sector and wanders less than half of it, the sigil's own
/// guarantee, so no rejection loop is needed and the derivation stays a pure function
/// of name and numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    stars: Vec<(f32, f32)>,
    states: Vec<StarState>,
}

impl Cluster {
    /// Stars a project with no history yet wears.
    pub const FLOOR: usize = 3;
    /// Shipped chunks per extra star, and the most extra stars history
    /// can add.
    pub const PER_STAR: u32 = 64;
    pub const CEILING: usize = 4;

    /// The ring the stars are placed on, as a share of the unit box's
    /// half-width, and how far a star may wander inside its own sector.
    const NEAR: f32 = 0.55;
    const FAR: f32 = 1.0;
    const JITTER: f32 = 0.22;
    /// The box is twice as wide as tall — the volume's clusters lie.
    const SQUASH: f32 = 0.5;

    /// The cluster for `project`.
    #[must_use]
    pub fn of(project: &GalaxyProject) -> Self {
        let shipped = Self::FLOOR
            + usize::try_from(project.done.unwrap_or(0) / Self::PER_STAR)
                .unwrap_or(Self::CEILING)
                .min(Self::CEILING);
        let mut states = vec![StarState::Done; shipped];
        if project.running > 0 {
            states.push(StarState::Executing);
        }
        if project.asking > 0 {
            states.push(StarState::Stuck);
        }
        let count = states.len();
        let mut bits = Seed::of(&project.name);
        let sector = std::f32::consts::TAU / count as f32;
        let stars: Vec<(f32, f32)> = (0..count)
            .map(|index| {
                let along = bits.unit();
                let out = bits.unit();
                let angle = (index as f32).mul_add(sector, (along - 0.5) * Self::JITTER * sector);
                let radius = (Self::FAR - Self::NEAR).mul_add(out, Self::NEAR);
                (
                    radius * libm::cosf(angle),
                    radius * libm::sinf(angle) * Self::SQUASH,
                )
            })
            .collect();
        // The chain visits the stars in their angular order — a loop
        // walked once, the way the volume's clusters are drawn. The
        // sigil shuffles its visiting order to tell four-star marks
        // apart at fourteen pixels; a cluster is two hundred wide and
        // its count and jitter already tell projects apart, and a
        // shuffled chain across seven stars is a tangle, not a
        // constellation.
        Self { stars, states }
    }

    /// Each star's centre in the unit box, in chain order.
    #[must_use]
    pub fn stars(&self) -> &[(f32, f32)] {
        &self.stars
    }

    /// Each star's state, in the same order — the live ones last.
    #[must_use]
    pub fn states(&self) -> &[StarState] {
        &self.states
    }

    /// The chain: every consecutive pair.
    #[must_use]
    pub fn edges(&self) -> Vec<(usize, usize)> {
        (1..self.stars.len()).map(|to| (to - 1, to)).collect()
    }
}

#[cfg(test)]
#[path = "cluster.tests.rs"]
mod tests;
