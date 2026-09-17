use std::fs::File;
use std::io::Read;

use super::*;

fn lock_file(tmp: &tempfile::TempDir) -> File {
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(tmp.path().join("owner.lock"))
        .expect("lock file")
}

#[test]
fn the_counter_starts_at_one_and_grows_in_place() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut lock = lock_file(&tmp);
    assert_eq!(mint(&mut lock).expect("first").get(), 1);
    assert_eq!(mint(&mut lock).expect("second").get(), 2);
    drop(lock);
    // A later process reads the same counter from the same file.
    let mut lock = lock_file(&tmp);
    let third = mint(&mut lock).expect("third");
    assert_eq!(third.get(), 3);
    let mut text = String::new();
    File::open(tmp.path().join("owner.lock"))
        .expect("reopen")
        .read_to_string(&mut text)
        .expect("read");
    assert_eq!(text, format!("{:020}", 3), "fixed width, nothing else");
}

#[test]
fn a_lock_file_that_holds_no_counter_starts_over() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("owner.lock"), b"not a number").expect("seed");
    let mut lock = lock_file(&tmp);
    assert_eq!(mint(&mut lock).expect("mint").get(), 1);
}
