use super::*;

#[test]
#[cfg(unix)]
fn a_cached_digest_detects_same_size_mtime_rewrites_and_atomic_replacements() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source");
    std::fs::write(&path, b"old").unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    let record = ManifestEntry {
        kind: super::super::EntryKind::Text,
        mode: 0o644,
        size: 3,
        digest: "old digest".into(),
    };
    put(&path, &metadata, &record);
    // Exercise the hit independently of the conservative two-second guard,
    // without sleeping or changing a process-wide clock.
    cache()
        .lock()
        .unwrap()
        .entries
        .get_mut(&path)
        .unwrap()
        .reusable = true;
    assert_eq!(get(&path, &metadata), Some(record.clone()));
    std::fs::write(&path, b"new").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(metadata.modified().unwrap())
        .unwrap();
    let changed = std::fs::metadata(&path).unwrap();
    assert_eq!(changed.len(), metadata.len());
    assert_eq!(changed.modified().unwrap(), metadata.modified().unwrap());
    assert_eq!(get(&path, &changed), None);
    assert!(!same(&metadata, &changed));
    let replacement = dir.path().join("replacement");
    std::fs::write(&replacement, b"old").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&replacement)
        .unwrap()
        .set_modified(metadata.modified().unwrap())
        .unwrap();
    std::fs::rename(replacement, &path).unwrap();
    assert_eq!(get(&path, &std::fs::metadata(&path).unwrap()), None);
}

#[test]
fn freshly_changed_files_are_not_reused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source");
    std::fs::write(&path, b"new").unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    put(&path, &metadata, &ManifestEntry::missing());
    assert_eq!(get(&path, &metadata), None);
}
