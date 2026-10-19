use serde::{Deserialize, Serialize};

/// A thing the session holds that is bound to the machine it runs on.
///
/// A snapshot's honesty lives here. Everything in [`SessionSnapshot`]
/// besides this list is a *value* — a status, a phase, a count — and a
/// value crosses a machine for free. What does not cross is the session's
/// grip on the host: the child process its agent speaks to, the terminals
/// that process opened, the exclusive lock it holds on a workspace, the
/// question it asked and nobody has answered.
///
/// The design choice is that these are **declared rather than omitted**.
/// A snapshot that simply left them out would restore into a session that
/// looks complete and is not; a snapshot that names them lets the restore
/// answer for each one. That is QEMU's unmigratable-device model one
/// lattice down: a device that cannot be moved blocks the migration by
/// saying so, rather than arriving as a silently wrong register file.
///
/// Two answers are possible, and [`is_blocking`] decides which.
///
/// *Severable* — the new host makes its own. It spawns its own agent
/// process, opens its own terminals, takes its own workspace lock. The
/// restore reports these, so the loss is on the record, and proceeds.
///
/// *Blocking* — the restore refuses. There is exactly one, and it is
/// [`PendingPermission`]: the answer to an unanswered permission request
/// would be delivered to a tool call inside a process that no longer
/// exists. A session restored believing a question is outstanding which
/// can never be answered is worse than one that refused to restore, so
/// the question must be answered or the turn cancelled before the
/// session can move.
///
/// [`SessionSnapshot`]: super::session_snapshot::SessionSnapshot
/// [`is_blocking`]: SessionResidency::is_blocking
/// [`PendingPermission`]: SessionResidency::PendingPermission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionResidency {
    /// The ACP child process the session's agent runs in.
    AgentProcess {
        /// The child's pid, when the host knew it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
    },
    /// A terminal the agent opened and still owns.
    Terminal {
        /// The terminal's id, as the agent knows it.
        id: String,
    },
    /// The exclusive per-workspace lock the workspace holds.
    WorkspaceLock {
        /// The workspace the lock is over.
        workspace: String,
    },
    /// A permission request the user has not answered.
    PendingPermission {
        /// The request the session is waiting on.
        request_id: String,
    },
}

impl SessionResidency {
    /// Whether this residency stops the session from being restored at
    /// all, rather than merely being left behind.
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::PendingPermission { .. })
    }

    /// The wire spelling, for status rows and refusal messages.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::AgentProcess { .. } => "agent_process",
            Self::Terminal { .. } => "terminal",
            Self::WorkspaceLock { .. } => "workspace_lock",
            Self::PendingPermission { .. } => "pending_permission",
        }
    }

    /// Why this cannot come along.
    #[must_use]
    pub fn reason(&self) -> &'static str {
        match self {
            Self::AgentProcess { .. } => {
                "the agent runs in a child process of the host; the new host spawns its own"
            }
            Self::Terminal { .. } => {
                "a terminal belongs to the process that opened it and dies with it"
            }
            Self::WorkspaceLock { .. } => {
                "the workspace lock is exclusive per workspace and is taken by the new host"
            }
            Self::PendingPermission { .. } => {
                "an unanswered permission would be delivered to a tool call in a process \
                 that no longer exists — answer it or cancel the turn before migrating"
            }
        }
    }

    /// One line naming this residency and why it stays behind.
    #[must_use]
    pub fn describe(&self) -> String {
        let detail = match self {
            Self::AgentProcess { pid: Some(pid) } => format!(" (pid {pid})"),
            Self::AgentProcess { pid: None } => String::new(),
            Self::Terminal { id } => format!(" '{id}'"),
            Self::WorkspaceLock { workspace } => format!(" on '{workspace}'"),
            Self::PendingPermission { request_id } => format!(" '{request_id}'"),
        };
        format!("{}{}: {}", self.kind(), detail, self.reason())
    }
}

#[cfg(test)]
#[path = "session_residency.tests.rs"]
mod tests;
