//! Re-export of [`magistr_core::child_guard`] for backward compatibility.
//!
//! The canonical implementation lives in `magistr-core` so both the CLI
//! and GUI crates share the same global PGID registry.

pub(crate) use magistr_core::child_guard::kill_all_children;
#[cfg(unix)]
pub(crate) use magistr_core::child_guard::register_child_pgid;
#[cfg(unix)]
pub(crate) use magistr_core::child_guard::unregister_child_pgid;
