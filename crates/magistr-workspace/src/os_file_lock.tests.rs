use super::*;
use std::io::Read;

#[test]
fn a_held_lock_refuses_a_second_owner_but_leaves_the_bytes_readable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owner.lock");
    std::fs::write(&path, b"marker").unwrap();
    let first = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    try_lock_exclusive(&first).unwrap();
    let second = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert_eq!(
        try_lock_exclusive(&second).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    let mut bytes = Vec::new();
    File::open(&path).unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"marker");
    assert_eq!(std::fs::read(&path).unwrap(), b"marker");
    unlock(&first).unwrap();
    try_lock_exclusive(&second).unwrap();
}
