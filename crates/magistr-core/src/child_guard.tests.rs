use super::*;
use magistr_process::ChildRegistry;

#[test]
fn child_guard_registry_uses_process_membership() {
    register_child_pgid(20001);
    register_child_pgid(20001);
    assert!(ChildRegistry::contains(20001));
    unregister_child_pgid(20001);
    assert!(!ChildRegistry::contains(20001));
}

#[test]
fn register_ignores_invalid_pgids() {
    for id in [-1, 0, 1] {
        register_child_pgid(id);
        assert!(!ChildRegistry::contains(id));
    }
}

#[test]
fn unregister_nonexistent_is_noop() {
    unregister_child_pgid(29999);
}
