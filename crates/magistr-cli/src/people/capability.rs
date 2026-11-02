//! What a team role is permitted to do.

/// A capability the role matrix grants or withholds.
///
/// This is the enforcement seam later multi-user slices hook into:
/// audit attribution, session ownership checks, and permission approval
/// all ask `role.can(capability)` rather than matching on roles directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Add or remove members and change non-Owner roles in the roster.
    ManageTeam,
    /// Promote members to Owner or demote Owners.
    TransferOwnership,
    /// Modify the roadmap: complete, add, or update chunks.
    EditRoadmap,
    /// Answer permission prompts for agent tool calls.
    ApprovePermissions,
    /// Start and drive agent sessions.
    RunSessions,
    /// Read sessions, the roadmap, and audit views.
    View,
}

impl Capability {
    /// Every capability, in display order.
    pub const ALL: [Capability; 6] = [
        Capability::ManageTeam,
        Capability::TransferOwnership,
        Capability::EditRoadmap,
        Capability::ApprovePermissions,
        Capability::RunSessions,
        Capability::View,
    ];

    /// Short human-readable label for CLI display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Capability::ManageTeam => "manage team",
            Capability::TransferOwnership => "transfer ownership",
            Capability::EditRoadmap => "edit roadmap",
            Capability::ApprovePermissions => "approve permissions",
            Capability::RunSessions => "run sessions",
            Capability::View => "view",
        }
    }
}
