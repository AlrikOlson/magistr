//! What a star on the sky is doing, in the design's own vocabulary.

/// The visual state of a roadmap node.
///
/// The renderer uses the state for its core, ring, and animation. Visible
/// status labels are provided separately by [`StateWord`](super::StateWord).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StarState {
    /// The roadmap has not arrived yet: a placeholder star, drawn faint
    /// and pulsing, that stands where a chunk will be.
    Ghost,
    /// Planned, but something it depends on is not shipped.
    Pending,
    /// Everything it depends on is shipped — the next thing that can be
    /// claimed. Pulses.
    Ready,
    /// A session is working it. Breathes.
    Executing,
    /// Working, but waiting on a decision. Breathes, and wears the
    /// alert ring.
    Stuck,
    /// Held by something outside the plan. Still, and wears the alert
    /// ring.
    Blocked,
    /// Shipped. Stays lit for ever, and twinkles at night.
    Done,
}

impl StarState {
    /// Every state, in the order specimen II.b lays them out — the
    /// order the state sheet is drawn in and the order a test walks.
    pub const ALL: [Self; 7] = [
        Self::Ghost,
        Self::Pending,
        Self::Ready,
        Self::Executing,
        Self::Stuck,
        Self::Blocked,
        Self::Done,
    ];

    /// The state a chunk with this wire `status` wears, given whether
    /// everything it depends on has shipped.
    ///
    /// `ready` only decides between [`StarState::Pending`] and
    /// [`StarState::Ready`]: every other status names itself. An
    /// unknown status reads as pending rather than as nothing, because
    /// the engines' wire shape is theirs to grow and a star the sky
    /// refused to draw would be a chunk the user cannot see.
    #[must_use]
    pub fn of(status: &str, ready: bool) -> Self {
        match status {
            "done" => Self::Done,
            "blocked" => Self::Blocked,
            "stuck" => Self::Stuck,
            "in_progress" => Self::Executing,
            _ if ready => Self::Ready,
            _ => Self::Pending,
        }
    }

    /// Whether the star twinkles when the sky is idle — night only, and
    /// only the shipped ones. The ghost's own fade is a pulse, not a
    /// twinkle: it says "loading", where a twinkle says "shipped".
    #[must_use]
    pub const fn twinkles(self) -> bool {
        matches!(self, Self::Done)
    }

    /// Whether the star's halo pulses outward — the ready star's ring,
    /// and the ghost's placeholder fade.
    #[must_use]
    pub const fn pulses(self) -> bool {
        matches!(self, Self::Ready | Self::Ghost)
    }

    /// Whether the star's core breathes — a session is on it.
    #[must_use]
    pub const fn breathes(self) -> bool {
        matches!(self, Self::Executing | Self::Stuck)
    }

    /// Whether the star's core is drawn at all, or only its outline.
    /// Pending and Ghost are outlines: nothing has happened yet.
    #[must_use]
    pub const fn has_core(self) -> bool {
        !matches!(self, Self::Pending | Self::Ghost)
    }

    /// Whether the star wears the red dashed alert ring — it needs a
    /// person. The one decoration that means the same thing on the
    /// constellation and on the sessions strip.
    #[must_use]
    pub const fn alerts(self) -> bool {
        matches!(self, Self::Stuck | Self::Blocked)
    }

    /// The state's name, as the design sheet labels it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ghost => "Ghost",
            Self::Pending => "Pending",
            Self::Ready => "Ready",
            Self::Executing => "Executing",
            Self::Stuck => "Stuck",
            Self::Blocked => "Blocked",
            Self::Done => "Done",
        }
    }
}

#[cfg(test)]
#[path = "star_state.tests.rs"]
mod tests;
