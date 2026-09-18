use super::*;

#[test]
fn a_loop_lease_refuses_a_second_owner_until_released() {
    let dir = tempfile::tempdir().unwrap();
    let lease = LoopLease::acquire(dir.path()).unwrap();
    assert!(LoopLease::acquire(dir.path()).is_err());
    drop(lease);
    assert!(LoopLease::acquire(dir.path()).is_ok());
}
