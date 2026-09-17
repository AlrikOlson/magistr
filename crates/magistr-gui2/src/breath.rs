//! `breathe` — the working thing, on the session stage.

use magistr_cockpit::cockpit::{Model, Motion, MotionName, Surface};
use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::medium::Medium;

/// The working breath: where it stands, `0..=1` up a raised cosine from rest to crest
/// and back, read off [`Model::now`] on the sky's own rhythm ([`Motion::GLOW_STEPS`])
/// so the cockpit has one breath — the working star on the Sky and the working card on
/// the stage rise and fall together.
///
/// It exists for every state in which the agent is doing something the cockpit cannot
/// see the inside of ([`Model::is_working`]): the wait before the first token,
/// thinking, a tool or a shell running, a file being written, and the loop's own gaps —
/// starting the agent, the gates, the judge. What breathes is the working *thing* (the
/// volume's words): a running tool or edit card wears the halo, a thought being thought
/// lifts its ink, and the stage cap's live dot covers the rest. A decision screen holds
/// still (a screen that waits on the human moves nothing), so a Moment up answers
/// `None`.
///
/// `None` too under reduced motion and on a terminal: the breath is the window's —
/// night: the halo swells; day: the compass ring pulses — and the terminal keeps its
/// quiet-loop contract.
///
/// The rest frame is *nothing*: at the foot of the wave a working card and the cap are
/// byte-identical to a cockpit that is not working, which is what keeps every settled
/// still (taken at a `GLOW_STEPS` foot) the still it was and the mock parity stills the
/// volume's own. The volume's `breathe7` keyframes draw a 4 px ring at rest; here the
/// ring is the crest's alone, so first law — still = settled, byte for byte — holds on
/// a screen the volume drew mid-work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    wave: f32,
}

impl Breath {
    /// The crest of `breathe7`: a 7 px ring at 24 % under a 30 px light
    /// at 28 %. [`Element::glow`] is one halo fading over its falloff,
    /// so the two stops become one light whose reach and weight both
    /// swell with the wave — the ring's edge at the foot, the light's
    /// reach at the crest.
    const REACH_PX: f32 = 30.0;
    const RING_PX: f32 = 7.0;
    const HALO_ALPHA: f32 = 0.28;
    const RING_ALPHA: f32 = 0.24;
    /// The ring's line: one pixel at the foot, two at the crest.
    const RING_WIDTH_PX: f32 = 1.0;
    /// Day's compass ring is the same outline in ink, not light (the
    /// volume's day column), at the weight ink needs to read on paper.
    const DAY_RING_ALPHA: f32 = 0.55;
    /// The live dot the volume draws beside the working thing: 9 px,
    /// mauve, its own halo reaching this far at the crest.
    pub const DOT_PX: f32 = 9.0;
    const DOT_REACH_PX: f32 = 12.0;

    /// The breath `model` is at, if it is breathing at all.
    #[must_use]
    pub fn of(model: &Model) -> Option<Self> {
        // The registry decides (`Model::moves`): full motion, the
        // working thing, and no decision screen up — the sixteen names
        // are the mechanism, so the stage and the vocabulary cannot
        // disagree about whether it breathes. The surface is asked
        // first because a terminal *breathes* too, in the registry's
        // reading — the spinner beside its streaming block — and that
        // one is the cell painter's, not this halo's.
        (model.surface() == Surface::Native && model.moves(MotionName::Breathe))
            .then(|| Self::at(Motion::glow_phase(model.now())))
    }

    /// The breath at `phase` of its period, `0..1` — `0.5` is the
    /// crest, which is what the sink exhibits.
    #[must_use]
    pub fn at(phase: f32) -> Self {
        Self { wave: wave(phase) }
    }

    /// How far up the breath is, `0` at the foot and `1` at the crest.
    #[must_use]
    pub const fn wave(self) -> f32 {
        self.wave
    }

    /// `card` — an opaque surface with corner `radius` — under this
    /// breath: the keyframe's ring, spreading out from the card's edge
    /// with the wave, and by night the light behind it.
    ///
    /// The ring is its own outline pinned *outside* the card (a stroke
    /// on the wrapper would sit under the card's ground), and it is
    /// what the CPU tier can rasterize — its glow floor is the solid
    /// shape, no halo — so the filmstrip and the parity stills see the
    /// breath the window sees. The light is a glow on the wrapper
    /// behind the card: the primitive paints its shape solid inside the
    /// halo and the card's ground covers that solid, so only the light
    /// outside the edge shows, on the GPU that draws it. Always
    /// wrapped, so the tree has one shape at the foot and off it.
    #[must_use]
    pub fn halo(self, card: Element, radius: f32, medium: Medium, lex: &Lexicon) -> Element {
        let mut wrap = Element::surface().radius(radius).child(card);
        if self.wave <= 0.0 {
            return wrap;
        }
        let width = Self::RING_WIDTH_PX.mul_add(self.wave, Self::RING_WIDTH_PX);
        let out = Self::RING_PX.mul_add(self.wave, width);
        let ink = match medium {
            Medium::Night => alpha(lex.glow_m, Self::RING_ALPHA * self.wave),
            Medium::Day => alpha(lex.mauve, Self::DAY_RING_ALPHA * self.wave),
        };
        let ring = Element::surface()
            .pin_left(-out)
            .pin_top(-out)
            .pin_right(-out)
            .pin_bottom(-out)
            .radius(radius + out)
            .stroke(ink, width);
        if medium == Medium::Night {
            wrap = wrap.glow(
                alpha(lex.glow_m, Self::HALO_ALPHA * self.wave),
                lerp(Self::RING_PX, Self::REACH_PX, self.wave),
            );
        }
        wrap.child(ring)
    }

    /// `dim` lifted toward `lit` by this breath — the ink of a thought
    /// being thought, breathing between the faint pen and the text pen
    /// (light over movement: nothing moves, the ink brightens).
    #[must_use]
    pub fn ink(self, dim: Srgba, lit: Srgba) -> Srgba {
        // The foot is `dim` itself, not a mix that rounds to it: the
        // settled still must be the faint pen byte for byte.
        if self.wave <= 0.0 {
            return dim;
        }
        // `mix` takes the share of its *first* colour: the foot is all
        // `dim`, the crest all `lit`.
        crate::oklab::mix(dim, lit, 1.0 - self.wave)
    }

    /// The live dot at this breath: the volume's mauve disc, coming up
    /// out of the ground with the wave and, by night, carrying its own
    /// light. Nothing at the foot.
    #[must_use]
    pub fn dot(self, medium: Medium, lex: &Lexicon) -> Element {
        let disc = Element::surface()
            .width(Self::DOT_PX)
            .height(Self::DOT_PX)
            .radius(Self::DOT_PX / 2.0)
            .fill(alpha(lex.mauve, self.wave));
        match medium {
            Medium::Night if self.wave > 0.0 => disc.glow(
                alpha(lex.glow_m, Self::HALO_ALPHA * self.wave),
                lerp(1.0, Self::DOT_REACH_PX, self.wave),
            ),
            Medium::Night | Medium::Day => disc,
        }
    }
}

/// `color` at `alpha`.
fn alpha(color: Srgba, alpha: f32) -> Srgba {
    Srgba::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
}

/// `from` at `t == 0` and `to` at `t == 1`.
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    (to - from).mul_add(t.clamp(0.0, 1.0), from)
}

/// A raised cosine over one period: `0` at both ends and `1` in the
/// middle — [`Motion::glow_phase`]'s own curve, the one the sky's
/// working star breathes on.
fn wave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 - 0.5 * libm::cosf(std::f32::consts::TAU * t)
}

#[cfg(test)]
#[path = "breath.tests.rs"]
mod tests;
