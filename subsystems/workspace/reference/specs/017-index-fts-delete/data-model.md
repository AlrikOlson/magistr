# Phase 1 Data Model: Index Build Without the Quadratic Delete

No new tables or columns. One new invariant and one stamp change in `.workbench/index.db`.

## Invariant

```
for every row f in files with content_indexed = 1:
  exactly one row c in content_fts with c.rowid = f.rowid and c.path = f.path
for every row f in files with content_indexed = 0:
  no row in content_fts with rowid = f.rowid
no row in content_fts whose rowid is not a files.rowid
```

Established by `writeFile` (files insert, then `INSERT INTO content_fts (rowid, path, content)
VALUES ((SELECT rowid FROM files WHERE path = ?1), ?1, ?2)`) and preserved by
`deleteFileRows` (FTS row by `(SELECT rowid FROM files WHERE path = ?)` first, `files` last).

## Schema stamp

```
PRAGMA user_version: 2 → 3   (INDEX_SCHEMA_VERSION)
```

A database stamped < 3 is discarded and rebuilt by `openVersionedDatabase` (cache-class, no
migration chain), as a stamp < 2 is today.

## `deleteFileRows(path)` contract

```
rowid := SELECT rowid FROM files WHERE path = ?
if rowid is null: return            (no rows of any kind exist for the path)
DELETE FROM content_fts WHERE rowid = ?rowid
DELETE FROM symbols     WHERE path = ?
DELETE FROM test_ranges WHERE path = ?
DELETE FROM imports     WHERE path = ?
DELETE FROM files       WHERE path = ?
```

Every statement is O(log n) (primary key, `symbols_path`, `test_ranges_path`, `imports_path`,
FTS row id).
