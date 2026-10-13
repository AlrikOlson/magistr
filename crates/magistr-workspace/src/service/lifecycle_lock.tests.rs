use super::*;

#[test]
fn source_owners_share_the_host_lease_and_block_archive_until_every_owner_closes() {
    let temp = tempfile::tempdir().expect("temporary");
    let path = temp.path().join("state/lifecycle.lock");
    let first = acquire(&path).expect("first source owner");
    let second = acquire(&path).expect("second source owner");
    let archive = File::options()
        .read(true)
        .write(true)
        .open(&path)
        .expect("archive");
    assert!(matches!(
        archive.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(first);
    assert!(archive.try_lock().is_err());
    drop(second);
    archive.try_lock().expect("closed owners");
    assert_eq!(
        acquire(&path).expect_err("archive in progress").kind(),
        io::ErrorKind::WouldBlock
    );
    drop(archive);
    acquire(&path).expect("reopened");
}
