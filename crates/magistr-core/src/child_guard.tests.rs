use super::*;

// Each test uses unique PGID values to avoid races with parallel tests
// sharing the global `CHILD_PGIDS` static.

#[test]
fn register_and_unregister() {
    register_child_pgid(20001);
    register_child_pgid(20002);

    {
        let pgids = CHILD_PGIDS.lock().unwrap();
        assert!(pgids.contains(&20001));
        assert!(pgids.contains(&20002));
    }

    unregister_child_pgid(20001);

    {
        let pgids = CHILD_PGIDS.lock().unwrap();
        assert!(!pgids.contains(&20001));
        assert!(pgids.contains(&20002));
    }

    unregister_child_pgid(20002);
}

#[test]
fn register_ignores_invalid_pgids() {
    register_child_pgid(0);
    register_child_pgid(-1);

    let pgids = CHILD_PGIDS.lock().unwrap();
    assert!(!pgids.contains(&0));
    assert!(!pgids.contains(&-1));
}

#[test]
fn register_is_idempotent() {
    register_child_pgid(20003);
    register_child_pgid(20003);

    {
        let pgids = CHILD_PGIDS.lock().unwrap();
        let count = pgids.iter().filter(|&&p| p == 20003).count();
        assert_eq!(count, 1);
    }

    unregister_child_pgid(20003);
}

#[test]
fn unregister_nonexistent_is_noop() {
    unregister_child_pgid(29999);
}

#[test]
fn kill_all_children_does_not_panic_when_empty() {
    kill_all_children();
}
