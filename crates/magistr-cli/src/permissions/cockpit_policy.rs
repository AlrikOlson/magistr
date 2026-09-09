//! Compatibility name for the shared session permission adapter.

/// Existing cockpit callers use the same policy owner as every CLI session.
pub type CockpitPolicy = super::session_permission_policy::SessionPermissionPolicy;
