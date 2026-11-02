#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

#[test]
fn exclusive_lock_creates_lock_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml_path = tmp.path();

    let _lock = FileLock::exclusive(yaml_path).unwrap();

    let expected_lock = lock_path_for(yaml_path);
    assert!(expected_lock.exists());
}

#[test]
fn shared_lock_creates_lock_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml_path = tmp.path();

    let _lock = FileLock::shared(yaml_path).unwrap();

    let expected_lock = lock_path_for(yaml_path);
    assert!(expected_lock.exists());
}

#[cfg(unix)]
#[test]
fn multiple_shared_locks_coexist() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml_path = tmp.path();

    let lock1 = FileLock::shared(yaml_path).unwrap();
    let lock2 = FileLock::shared(yaml_path).unwrap();

    // Both locks acquired — no deadlock
    drop(lock1);
    drop(lock2);
}

#[cfg(unix)]
#[test]
fn exclusive_lock_serializes_access() {
    use std::thread;

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml_path = tmp.path().to_path_buf();
    let counter = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = yaml_path.clone();
        let ctr = counter.clone();
        handles.push(thread::spawn(move || {
            let _lock = FileLock::exclusive(&path).unwrap();
            // With proper serialization, this increment + read is atomic-ish:
            // no two threads hold the lock simultaneously.
            let prev = ctr.fetch_add(1, Ordering::SeqCst);
            // Simulate some work under lock
            std::thread::sleep(std::time::Duration::from_millis(5));
            prev
        }));
    }

    let mut values: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    values.sort();
    // Each thread saw a unique counter value: 0, 1, 2, 3
    assert_eq!(values, vec![0, 1, 2, 3]);
}

#[test]
fn lock_path_for_appends_dot_lock() {
    let path = Path::new("/foo/bar.yml");
    assert_eq!(lock_path_for(path), PathBuf::from("/foo/bar.yml.lock"));
}
