//! One project as the Galaxy knows it.

/// One project on the Galaxy — altitude 0, where every
/// project is a cluster. What this machine knows about it, as numbers:
/// the copy ("211 done · 1 running", "Idle · 3 days") is the surface's
/// to set, so the cockpit stays surface-agnostic and the words stay in
/// one place.
///
/// Gathered off the loop thread by the entry point (the session
/// database, the project's own store) and delivered with the rest of
/// the Galaxy's data as one
/// [`JobOutput::Galaxy`](crate::job::JobOutput::Galaxy).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GalaxyProject {
    /// The project's name (its config's, or the directory's).
    pub name: String,
    /// The directory the project lives in, when it is known.
    pub dir: Option<String>,
    /// Whether this is the window's own project — the one whose
    /// engines this window is rooted on, and the one a dive can open.
    pub here: bool,
    /// Chunks shipped, when the project's store could be read — the
    /// cluster's density.
    pub done: Option<u32>,
    /// Live sessions on the project — each one breathes.
    pub running: u32,
    /// Decisions waiting on a person — each one glows amber.
    pub asking: u32,
    /// How long the oldest question has waited, in seconds.
    pub asked_secs: Option<u64>,
    /// How long since the project last had a session, in seconds —
    /// what an idle cluster says.
    pub idle_secs: Option<u64>,
}

impl GalaxyProject {
    /// A project called `name`, with nothing else known yet.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Whether anything is happening on the project — a session
    /// running or a question pending.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.running > 0 || self.asking > 0
    }

    /// Whether a decision is waiting on a person.
    #[must_use]
    pub const fn is_asking(&self) -> bool {
        self.asking > 0
    }
}
