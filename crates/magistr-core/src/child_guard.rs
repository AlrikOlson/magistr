//! Compatibility access to the process mechanism's shared hard-exit registry.

/// Register an isolated host-owned process group for cleanup on exit.
pub fn register_child_pgid(pgid: i32) {
    magistr_process::ChildRegistry::register(pgid);
}

/// Unregister a process group after confirmed cleanup.
pub fn unregister_child_pgid(pgid: i32) {
    magistr_process::ChildRegistry::unregister(pgid);
}

/// Request termination of all registered groups during host shutdown.
pub fn kill_all_children() {
    magistr_process::ChildRegistry::kill_all();
}

#[cfg(test)]
#[path = "child_guard.tests.rs"]
mod tests;
