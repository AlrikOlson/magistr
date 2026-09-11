use super::*;
use crate::worker::{WorkerSymbol, WorkerTestRange};

fn sample(path: &str, name: &str) -> IndexedFile {
    IndexedFile {
        path: path.to_owned(),
        language: Some("rust".to_owned()),
        size: 21,
        mtime_ms: 100,
        mode: 0o644,
        content_hash: "abc123".to_owned(),
        junk: false,
        content: Some(format!("fn {name}() {{}} // needle")),
        symbols: vec![WorkerSymbol {
            name: name.to_owned(),
            kind: "function".to_owned(),
            start_line: 1,
            start_col: 1,
            end_line: 1,
            detail: Some("Example".to_owned()),
            doc: Some("A doc".to_owned()),
        }],
        imports: vec!["crate::other".to_owned()],
        test_ranges: vec![WorkerTestRange {
            start_line: 2,
            end_line: 4,
        }],
    }
}

#[test]
fn roundtrip_reopen_and_generation() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample("src/lib.rs", "hello");
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    assert_eq!(cache.tier().unwrap(), 0);
    cache.update(&file).unwrap();
    assert_eq!(cache.file(&file.path).unwrap(), Some(file.clone()));
    let generation = cache.finish_build().unwrap();
    drop(cache);
    let cache = IndexCache::open(dir.path(), "WAL").unwrap();
    assert_eq!(cache.tier().unwrap(), 1);
    assert_eq!(cache.generation().unwrap(), generation);
    assert_eq!(cache.file(&file.path).unwrap(), Some(file));
}

#[test]
fn mutations_keep_fts_rowids_aligned_without_stale_terms() {
    let dir = tempfile::tempdir().unwrap();
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    cache.update(&sample("a.rs", "former")).unwrap();
    cache.update(&sample("b.rs", "other")).unwrap();
    cache.update(&sample("a.rs", "current")).unwrap();
    assert!(cache.fts("former", 10).unwrap().is_empty());
    assert_eq!(cache.fts("current", 10).unwrap()[0].path, "a.rs");
    cache.remove("a.rs").unwrap();
    cache.update(&sample("c.rs", "replacement")).unwrap();
    assert!(cache.fts("current", 10).unwrap().is_empty());
    assert_eq!(
        cache.counts().unwrap(),
        CacheCounts {
            files: 2,
            symbols: 2,
            imports: 2,
            test_ranges: 2,
            content: 2
        }
    );
    let db = rusqlite::Connection::open(dir.path().join("native-index.db")).unwrap();
    let mismatches: u64 = db.query_row("SELECT count(*) FROM content_fts c LEFT JOIN files f ON f.rowid=c.rowid WHERE f.path IS NULL OR f.path!=c.path", [], |row| row.get(0)).unwrap();
    assert_eq!(mismatches, 0);
}

#[test]
fn query_results_are_bounded_literal_and_metadata_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    for n in 0..1005 {
        cache
            .update(&sample(&format!("{n:04}.rs"), &format!("hello_{n}")))
            .unwrap();
    }
    cache
        .update(&sample("literal.rs", "hello%_literal"))
        .unwrap();
    assert_eq!(cache.files_page(None, usize::MAX).unwrap().len(), 1000);
    assert_eq!(cache.files_page(Some("0999.rs"), 1000).unwrap().len(), 6);
    let symbols = cache.symbols("HELLO", usize::MAX).unwrap();
    assert_eq!(symbols.len(), 1000);
    assert_eq!(cache.symbols("hello%_", 5).unwrap().len(), 1);
    assert!(cache.symbols("ello", 5).unwrap().is_empty());
    assert!(cache.files_page(None, 0).unwrap().is_empty());
    let hits = cache.fts("needle", usize::MAX).unwrap();
    assert_eq!(hits.len(), 1000);
    assert!(
        hits.iter()
            .all(|file| file.content.is_none() && file.symbols.is_empty())
    );
    assert!(cache.fts("needle OR nonexistent", 10).unwrap().is_empty());
    assert!(matches!(
        cache.fts(&"x".repeat(1025), 10),
        Err(CacheError::InvalidInput(_))
    ));
}

#[test]
fn corrupt_and_incompatible_caches_rebuild_without_touching_retained_state() {
    let dir = tempfile::tempdir().unwrap();
    let native = dir.path().join("native-index.db");
    let retained = dir.path().join("index.db");
    std::fs::write(&retained, b"retained reference").unwrap();
    std::fs::write(&native, b"broken database").unwrap();
    let cache = IndexCache::open(dir.path(), "TRUNCATE").unwrap();
    assert_eq!(cache.counts().unwrap().files, 0);
    assert_eq!(cache.journal_mode, "TRUNCATE");
    assert!(cache.rebuild_reason.as_deref().unwrap().contains("corrupt"));
    drop(cache);
    let db = rusqlite::Connection::open(&native).unwrap();
    assert_eq!(
        db.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "delete"
    );
    db.execute_batch("PRAGMA user_version=999; CREATE TABLE obsolete(x)")
        .unwrap();
    drop(db);
    let cache = IndexCache::open(dir.path(), "TRUNCATE").unwrap();
    assert_eq!(cache.tier().unwrap(), 0);
    assert_eq!(std::fs::read(retained).unwrap(), b"retained reference");
}

#[test]
fn schema_mismatch_rebuilds_and_operational_errors_do_not_delete() {
    let dir = tempfile::tempdir().unwrap();
    let native = dir.path().join("native-index.db");
    let cache = IndexCache::open(dir.path(), "WAL").unwrap();
    drop(cache);
    let db = rusqlite::Connection::open(&native).unwrap();
    db.execute_batch("DROP TABLE symbols").unwrap();
    drop(db);
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    cache.update(&sample("a.rs", "unchanged")).unwrap();
    drop(cache);
    assert!(IndexCache::open(dir.path(), "OFF").is_err());
    assert_eq!(
        IndexCache::open(dir.path(), "WAL")
            .unwrap()
            .counts()
            .unwrap()
            .files,
        1
    );
    let locked = rusqlite::Connection::open(&native).unwrap();
    locked
        .execute_batch("PRAGMA journal_mode=DELETE; BEGIN EXCLUSIVE")
        .unwrap();
    assert!(IndexCache::open(dir.path(), "WAL").is_err());
    locked.execute_batch("ROLLBACK").unwrap();
    drop(locked);
    assert_eq!(
        IndexCache::open(dir.path(), "WAL")
            .unwrap()
            .counts()
            .unwrap()
            .files,
        1
    );
}

#[cfg(unix)]
#[test]
fn database_and_sidecar_links_cannot_alias_durable_files() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let durable = dir.path().join("journal.db");
    std::fs::write(&durable, b"durable bytes").unwrap();
    let native = dir.path().join("native-index.db");
    symlink(&durable, &native).unwrap();
    assert!(IndexCache::open(dir.path(), "WAL").is_err());
    std::fs::remove_file(&native).unwrap();
    std::fs::hard_link(&durable, &native).unwrap();
    assert!(IndexCache::open(dir.path(), "WAL").is_err());
    std::fs::remove_file(&native).unwrap();
    symlink(&durable, dir.path().join("native-index.db-wal")).unwrap();
    assert!(IndexCache::open(dir.path(), "WAL").is_err());
    assert_eq!(std::fs::read(durable).unwrap(), b"durable bytes");
}

#[test]
fn unexpected_trigger_is_rebuilt_before_it_can_run() {
    let dir = tempfile::tempdir().unwrap();
    drop(IndexCache::open(dir.path(), "WAL").unwrap());
    let db = rusqlite::Connection::open(dir.path().join("native-index.db")).unwrap();
    db.execute_batch(
        "CREATE TRIGGER unexpected AFTER INSERT ON files BEGIN DELETE FROM symbols; END;",
    )
    .unwrap();
    drop(db);
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    assert!(
        cache
            .rebuild_reason
            .as_deref()
            .unwrap()
            .contains("triggers")
    );
    cache.update(&sample("a.rs", "kept")).unwrap();
    assert_eq!(cache.symbols("kept", 1).unwrap().len(), 1);
}

#[test]
fn live_owner_refuses_second_open_without_rebuilding_and_releases_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let durable = dir.path().join("journal.db");
    std::fs::write(&durable, b"durable state").unwrap();
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    cache.update(&sample("a.rs", "owner")).unwrap();
    // Even a deliberately incompatible schema must not be rebuilt while the
    // connection owning the native index is still alive.
    cache
        .db
        .execute_batch("PRAGMA user_version=999; PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let native = dir.path().join("native-index.db");
    let bytes = std::fs::read(&native).unwrap();
    assert!(
        matches!(IndexCache::open(dir.path(), "WAL"), Err(CacheError::Io(error)) if error.kind()==std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(std::fs::read(&native).unwrap(), bytes);
    assert_eq!(std::fs::read(&durable).unwrap(), b"durable state");
    cache.db.execute_batch("PRAGMA user_version=1").unwrap();
    drop(cache);
    let cache = IndexCache::open(dir.path(), "WAL").unwrap();
    assert_eq!(cache.counts().unwrap().files, 1);
}

#[cfg(unix)]
#[test]
fn dropping_cache_releases_lock_even_while_a_duplicated_descriptor_is_alive() {
    let dir = tempfile::tempdir().unwrap();
    let mut cache = IndexCache::open(dir.path(), "WAL").unwrap();
    cache.update(&sample("a.rs", "retained")).unwrap();
    // dup and fork share a flock ownership reference. Model a concurrently
    // spawned child's pre-exec descriptor without forking this test process.
    let inherited = cache._owner.0.try_clone().unwrap();
    assert!(
        matches!(IndexCache::open(dir.path(), "WAL"), Err(CacheError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    drop(cache);
    let reopened = IndexCache::open(dir.path(), "WAL").expect(
        "the completed cache owner must release its lock before inherited descriptors close",
    );
    assert_eq!(reopened.counts().unwrap().files, 1);
    assert!(inherited.metadata().is_ok());
    drop(inherited);
}

#[test]
fn native_lock_rejects_hardlinks_to_durable_state() {
    let dir = tempfile::tempdir().unwrap();
    let durable = dir.path().join("journal.db");
    std::fs::write(&durable, b"durable state").unwrap();
    std::fs::hard_link(&durable, dir.path().join("native-index.lock")).unwrap();
    assert!(IndexCache::open(dir.path(), "WAL").is_err());
    assert_eq!(std::fs::read(&durable).unwrap(), b"durable state");
}

#[cfg(unix)]
#[test]
fn native_lock_rejects_symlinks_to_durable_state() {
    let dir = tempfile::tempdir().unwrap();
    let durable = dir.path().join("journal.db");
    std::fs::write(&durable, b"durable state").unwrap();
    std::os::unix::fs::symlink(&durable, dir.path().join("native-index.lock")).unwrap();
    assert!(IndexCache::open(dir.path(), "WAL").is_err());
    assert_eq!(std::fs::read(&durable).unwrap(), b"durable state");
}
