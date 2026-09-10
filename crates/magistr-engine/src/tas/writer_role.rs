//! Which kind of admitted writer a [`super::ProjectAuthority`] is.

/// The writer class an owner lease admits. Each role holds its own lock, so
/// one writer of each role can be admitted on the same project and store at
/// the same time — the cockpit or loop host beside the agent's own
/// `magistr mcp-serve` — while a second writer of the *same* role is refused.
///
/// This is cooperative admission among Magistr binaries that take the lease.
/// It does not exclude older binaries, a standalone think-and-ship server, or
/// anyone who edits the store or its lock files directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriterRole {
    /// The MCP transport (`magistr mcp-serve`): the agent's tools, and the
    /// only role that publishes the live feed.
    Service,
    /// An interactive or headless host: the cockpit's `run`, `next` and
    /// `next-loop`, the plain loop, and one-shot CLI mutations.
    Host,
}

impl WriterRole {
    /// The lock file name this role contends on, under the persistent store
    /// (`<data_dir>/roadmap/sessions/<project_id>.<suffix>`) or the project's
    /// `.magistr/` when persistence is off. The service suffix predates roles
    /// and is kept so older service binaries still contend with new ones.
    #[must_use]
    pub const fn lock_suffix(self) -> &'static str {
        match self {
            Self::Service => "mcp-owner.lock",
            Self::Host => "host-owner.lock",
        }
    }

    /// What a refusal names: the resource another writer already holds.
    #[must_use]
    pub const fn resource(self) -> &'static str {
        match self {
            Self::Service => "project owner",
            Self::Host => "project host owner",
        }
    }
}
