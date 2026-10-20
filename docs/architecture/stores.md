# SQLite stores

Magistr links one SQLite. `rusqlite` with the `bundled` feature compiles the
library into every crate that uses it (`magistr-workspace`, and `magistr-cli`
through it and directly), so `Cargo.lock` resolves one `libsqlite3-sys` and one
runtime version for every store below. The floor is SQLite 3.51.3: the
[WAL-reset bug](https://www.sqlite.org/wal.html#the_wal_reset_bug) — a
checkpoint racing a commit that resets the WAL, corrupting the database — is
present from 3.7.0 through 3.51.2 and fixed in 3.51.3 (2026-03-13). The bug
needs two or more connections on one WAL-mode file, in separate threads or
processes, writing and checkpointing at the same time. One connection, a
non-WAL mode, or checkpoints that never overlap a reset cannot trigger it.

Two tests pin the floor rather than the lock file: they assert that
`rusqlite::version_number()` is at least 3051003, that the runtime equals the
compiled bundled header, and that `SELECT sqlite_version()` through each
store's own connection reports the same library
(`journal_db.tests.rs::bundled_sqlite_runtime_carries_the_wal_reset_fix`,
`storage.tests.rs::sessions_store_runs_on_a_sqlite_with_the_wal_reset_fix`).
Today that runtime is SQLite 3.53.2 (`rusqlite` 0.40.2, `libsqlite3-sys`
0.38.2).

## Inventory

| Store | File | Journal mode | Writers | Checkpointers | Readers | Advisory |
|---|---|---|---|---|---|---|
| Workspace journal | `.magistr/workspace/journal.db` | `WAL`, or `TRUNCATE` on a network filesystem; the applied mode is read back | One: the admitted `WorkspaceService` (`JournalDb`, exclusive `.magistr/workspace/server.lock`) | The same connection, and only it | `JournalReader` and doctor probes open `SQLITE_OPEN_READ_ONLY`; a bridge attaches to the owner instead of opening the file | Outside: one writer that is the sole checkpointer; read-only connections cannot write or reset the WAL |
| Workspace index cache | `.magistr/workspace/native-index.db` | `WAL`, or `TRUNCATE` on a network filesystem; `synchronous=NORMAL` | One: `IndexCache` behind the exclusive `native-index.lock`; a second opener is refused | The same connection | A read-only probe before open and the doctor's read-only health probe; the lock is never taken by readers | Outside: single connection; rebuildable cache data |
| Sessions store | `$XDG_DATA_HOME/magistr/sessions.db` (default `~/.local/share/magistr/sessions.db`) | `WAL`, set by every opener | Every `open_db` caller holds a read-write connection: `magistr sessions` (list, tag, purge, handoff) and the home screen's session query; `open_db` itself runs pending migrations | Every read-write connection auto-checkpoints on commit | Nothing opens it read-only | Inside before this baseline: two processes (a home screen beside a `sessions` command) hold read-write WAL connections and each may write and checkpoint. Fixed by the runtime floor; a controlled fixture drives four connections through overlapping inserts and `wal_checkpoint(TRUNCATE)` and verifies `integrity_check` |
| Plugin storage | Caller-named path | `WAL`, set per operation | `SqliteStorage` opens one short-lived connection per store, load, list or delete | That connection | None besides those operations | Outside as used: no production composition opens it; exercised by its own tests only |

Not SQLite: the cost ledger is a JSON file rewritten whole by
`cost::append_entry` during shared session settlement; each logical conversation
has one entry with its latest cumulative reported USD amount, or an unknown price.
Native workflow state (roadmap, reasoning, execution,
signals) is the workflow engine's own store; `.magistr/loop-state.json` is a
JSON checkpoint. A `.magistr/roadmap.db` left by the retired standalone
workflow server is not opened by any current code.

## Guarantees and their evidence

- **Journal.** Admission takes the OS lock before the journal opens, so a
  process holds at most one writer and no other process can open a second.
  Readers are true read-only connections: `read_only_reader_beside_the_sole_writer_reads_across_a_checkpoint`
  shows a reader observing new commits, a read-only connection refused with
  `SQLITE_READONLY`, and the writer's `wal_checkpoint(TRUNCATE)` completing
  with both views intact. Migration fixtures (`pinned_v1_and_v2_sqlite_history_migrates_without_rewriting_blobs_or_records`,
  `first_intent_migrates_atomically_without_rewriting_retained_blobs`) keep
  currently read reference-produced files. The automatic-upgrade policy additionally
  requires existing installations to retain their durable history during conversion.
  The frozen `fixtures/upgrades/workbench-v3.sql` schema exercises that admission path.
- **Index cache.** `live_owner_refuses_second_open_without_rebuilding_and_releases_on_drop`
  proves the single-connection topology; corruption rebuilds only the four
  expendable cache paths.
- **Sessions.** The runtime floor is the fix; nothing in the topology changed.
  `concurrent_wal_writers_and_checkpointers_leave_the_sessions_store_intact`
  runs the advisory's shape under control and checks integrity afterwards.
  `pre_owner_database_migrates_and_reads_owner_none` covers the
  `user_version` chain on an older file.
- **Integer checks.** `rusqlite` 0.38 stopped converting `u64` columns
  implicitly. The index cache reads counts and sizes as `i64` and converts
  with `u64::try_from`, refusing a negative value as
  `IntegralValueOutOfRange` exactly as the earlier implicit conversion did;
  the file size parameter is bounded with `i64::try_from` before it is bound.

## Retained reference source

The Bun reference under `subsystems/workspace/reference` carries its own
SQLite through `bun:sqlite` and is pinned by
[provenance](../../subsystems/workspace/PROVENANCE.md). This baseline changes
nothing there; the reference gate still decodes native journals with the
pinned reference reader.
