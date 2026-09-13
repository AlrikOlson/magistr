//! Private native SQLite cache; retained Workbench and durable state are untouched.

mod cache_counts;
mod cache_error;
mod cache_owner;
mod cached_symbol;
mod indexed_file;

pub use cache_counts::CacheCounts;
pub use cache_error::CacheError;
pub use cached_symbol::CachedSymbol;
pub use indexed_file::IndexedFile;

use cache_owner::CacheOwner;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};

use crate::worker::{WorkerSymbol, WorkerTestRange};

const VERSION: i64 = 1;
const MAX_ROWS: usize = 1000;
const MAX_QUERY_BYTES: usize = 1024;
const METADATA_COLUMNS: &str = "f.path,f.language,f.size,f.mtime_ms,f.mode,f.content_hash,f.junk";
const SCHEMA: &[(&str, &str)] = &[
    (
        "meta",
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL)",
    ),
    (
        "files",
        "CREATE TABLE files (path TEXT PRIMARY KEY, language TEXT, size INTEGER NOT NULL, mtime_ms INTEGER NOT NULL, mode INTEGER NOT NULL, content_hash TEXT NOT NULL, junk INTEGER NOT NULL, content_indexed INTEGER NOT NULL)",
    ),
    (
        "symbols",
        "CREATE TABLE symbols (path TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, start_line INTEGER NOT NULL, start_col INTEGER NOT NULL, end_line INTEGER NOT NULL, detail TEXT, doc TEXT)",
    ),
    (
        "symbols_name",
        "CREATE INDEX symbols_name ON symbols (name COLLATE NOCASE)",
    ),
    (
        "symbols_path",
        "CREATE INDEX symbols_path ON symbols (path)",
    ),
    (
        "imports",
        "CREATE TABLE imports (path TEXT NOT NULL, target TEXT NOT NULL)",
    ),
    (
        "imports_path",
        "CREATE INDEX imports_path ON imports (path)",
    ),
    (
        "test_ranges",
        "CREATE TABLE test_ranges (path TEXT NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL)",
    ),
    (
        "test_ranges_path",
        "CREATE INDEX test_ranges_path ON test_ranges (path)",
    ),
    (
        "content_fts",
        "CREATE VIRTUAL TABLE content_fts USING fts5(path UNINDEXED, content)",
    ),
];

/// Single connection owned by the index coordinator, with bounded query results.
pub struct IndexCache {
    db: Connection,
    // Rust drops fields in declaration order: SQLite closes and checkpoints
    // before this native-only ownership lock is released. Never unlink it.
    _owner: CacheOwner,
    /// Why an expendable native cache was rebuilt, if it was incompatible.
    pub rebuild_reason: Option<String>,
    /// Journal mode actually accepted by SQLite for this connection.
    pub journal_mode: String,
}

impl IndexCache {
    pub fn open(state_dir: &Path, journal_mode: &str) -> Result<Self, CacheError> {
        if !matches!(journal_mode, "WAL" | "TRUNCATE") {
            return Err(CacheError::InvalidInput(
                "journal mode must be WAL or TRUNCATE".to_owned(),
            ));
        }
        std::fs::create_dir_all(state_dir)?;
        // Resolve legitimate ancestor aliases (for example macOS /var) before
        // asking SQLite to reject symlinks anywhere in the database path.
        let state_dir = state_dir.canonicalize()?;
        let owner = lock_native_cache(&state_dir.join("native-index.lock"))?;
        let path = state_dir.join("native-index.db");
        let paths = cache_paths(&path);
        for candidate in &paths {
            check_cache_path(candidate)?;
        }
        let rebuild_reason = if path.exists() {
            match probe(&path) {
                Ok(reason) => reason,
                Err(CacheError::Sqlite(error)) if is_corruption(&error) => {
                    Some(format!("corrupt native cache: {error}"))
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        if rebuild_reason.is_some() {
            // Only these four expendable native-cache paths are ever removed.
            // Busy, read-only, permission and other operational errors above
            // never reach this branch.
            for candidate in &paths {
                check_cache_path(candidate)?;
                match std::fs::remove_file(candidate) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        let fresh = !path.exists();
        let mut db = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        db.busy_timeout(Duration::from_millis(100))?;
        let actual: String =
            db.query_row(&format!("PRAGMA journal_mode={journal_mode}"), [], |row| {
                row.get(0)
            })?;
        db.execute_batch("PRAGMA synchronous=NORMAL")?;
        if fresh {
            let transaction = db.transaction()?;
            for (_, sql) in SCHEMA {
                transaction.execute_batch(sql)?;
            }
            transaction.execute_batch(
                "INSERT INTO meta VALUES ('generation',0),('tier',0); PRAGMA user_version=1",
            )?;
            transaction.commit()?;
        }
        Ok(Self {
            db,
            _owner: owner,
            rebuild_reason,
            journal_mode: actual.to_uppercase(),
        })
    }

    /// File metadata alone, for reuse and freshness decisions without content I/O.
    pub fn metadata(&self, path: &str) -> Result<Option<IndexedFile>, CacheError> {
        Ok(self
            .db
            .query_row(
                &format!("SELECT {METADATA_COLUMNS} FROM files f WHERE f.path=?"),
                [path],
                metadata_row,
            )
            .optional()?)
    }

    /// Hydrate one file's extraction and content, addressed through its files rowid.
    pub fn file(&self, path: &str) -> Result<Option<IndexedFile>, CacheError> {
        let Some(mut file) = self.metadata(path)? else {
            return Ok(None);
        };
        file.content = self.db.query_row("SELECT content FROM content_fts WHERE rowid=(SELECT rowid FROM files WHERE path=?)", [path], |row| row.get(0)).optional()?;
        file.symbols = self.db.prepare_cached("SELECT name,kind,start_line,start_col,end_line,detail,doc FROM symbols WHERE path=? ORDER BY start_line,start_col,name")?.query_map([path], |row| symbol_row(row, 0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        file.imports = self
            .db
            .prepare_cached("SELECT target FROM imports WHERE path=? ORDER BY rowid")?
            .query_map([path], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        file.test_ranges = self.db.prepare_cached("SELECT start_line,end_line FROM test_ranges WHERE path=? ORDER BY start_line,end_line")?.query_map([path], |row| Ok(WorkerTestRange { start_line: row.get(0)?, end_line: row.get(1)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Some(file))
    }

    /// Which of `paths` the files table does not know at all — a membership
    /// probe in one statement (no rows hydrated, nothing validated) so a
    /// bounded query can still say what it could not see.
    pub fn unknown_paths(&self, paths: &[&str]) -> Result<Vec<String>, CacheError> {
        let listed = serde_json::to_string(paths)
            .map_err(|error| CacheError::InvalidInput(error.to_string()))?;
        Ok(self
            .db
            .prepare_cached(
                "SELECT value FROM json_each(?1) WHERE value NOT IN (SELECT path FROM files)",
            )?
            .query_map([listed], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Keyset pagination, capped at 1000 metadata rows per call.
    pub fn files_page(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<IndexedFile>, CacheError> {
        Ok(self.db.prepare_cached(&format!("SELECT {METADATA_COLUMNS} FROM files f WHERE (?1 IS NULL OR f.path>?1) ORDER BY f.path LIMIT ?2"))?.query_map(params![after, bounded(limit)], metadata_row)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Literal case-insensitive name prefix; SQL wildcard characters stay literal.
    pub fn symbols(&self, query: &str, limit: usize) -> Result<Vec<CachedSymbol>, CacheError> {
        validate_query(query)?;
        let prefix = format!(
            "{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        Ok(self.db.prepare_cached(&format!("SELECT {METADATA_COLUMNS},s.name,s.kind,s.start_line,s.start_col,s.end_line,s.detail,s.doc FROM symbols s JOIN files f ON f.path=s.path WHERE s.name LIKE ?1 ESCAPE '\\' COLLATE NOCASE ORDER BY s.name COLLATE NOCASE,s.path,s.start_line,s.start_col LIMIT ?2"))?.query_map(params![prefix,bounded(limit)], |row| Ok(CachedSymbol { file: metadata_row(row)?, symbol: symbol_row(row, 7)? }))?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Literal FTS phrase candidates — the first `limit` matches in index
    /// order, a bounded probe that stops as soon as the bound is met (the
    /// retained candidate query orders nothing, and ordering by path would
    /// force every match of a hot name to be evaluated before the cap could
    /// apply). Callers verify live content before presenting hits.
    pub fn fts(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>, CacheError> {
        validate_query(query)?;
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let phrase = format!("content:\"{}\"", query.replace('"', "\"\""));
        Ok(self.db.prepare_cached(&format!("SELECT {METADATA_COLUMNS} FROM content_fts JOIN files f ON f.rowid=content_fts.rowid WHERE content_fts MATCH ?1 LIMIT ?2"))?.query_map(params![phrase,bounded(limit)], metadata_row)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Replace every row of a file atomically, including its aligned FTS row.
    pub fn update(&mut self, file: &IndexedFile) -> Result<(), CacheError> {
        let size = i64::try_from(file.size).map_err(|_| {
            CacheError::InvalidInput("file size exceeds SQLite integer range".to_owned())
        })?;
        let transaction = self.db.transaction()?;
        delete_file(&transaction, &file.path)?;
        transaction.execute("INSERT INTO files (path,language,size,mtime_ms,mode,content_hash,junk,content_indexed) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![file.path,file.language,size,file.mtime_ms,file.mode,file.content_hash,file.junk,file.content.is_some()])?;
        if let Some(content) = &file.content {
            transaction.execute("INSERT INTO content_fts (rowid,path,content) VALUES ((SELECT rowid FROM files WHERE path=?1),?1,?2)", params![file.path,content])?;
        }
        for symbol in &file.symbols {
            transaction.prepare_cached("INSERT INTO symbols (path,name,kind,start_line,start_col,end_line,detail,doc) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")?.execute(params![file.path,symbol.name,symbol.kind,symbol.start_line,symbol.start_col,symbol.end_line,symbol.detail,symbol.doc])?;
        }
        for target in &file.imports {
            transaction
                .prepare_cached("INSERT INTO imports (path,target) VALUES (?1,?2)")?
                .execute(params![file.path, target])?;
        }
        for range in &file.test_ranges {
            transaction
                .prepare_cached(
                    "INSERT INTO test_ranges (path,start_line,end_line) VALUES (?1,?2,?3)",
                )?
                .execute(params![file.path, range.start_line, range.end_line])?;
        }
        bump_generation(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn remove(&mut self, path: &str) -> Result<(), CacheError> {
        let transaction = self.db.transaction()?;
        delete_file(&transaction, path)?;
        bump_generation(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn generation(&self) -> Result<u64, CacheError> {
        Ok(self
            .db
            .query_row("SELECT value FROM meta WHERE key='generation'", [], |row| {
                unsigned(row, 0)
            })?)
    }

    pub fn tier(&self) -> Result<u8, CacheError> {
        Ok(self
            .db
            .query_row("SELECT value FROM meta WHERE key='tier'", [], |row| {
                row.get(0)
            })?)
    }

    /// Persist that a complete build succeeded; progress timestamps remain process-local.
    pub fn finish_build(&mut self) -> Result<u64, CacheError> {
        let transaction = self.db.transaction()?;
        transaction.execute("UPDATE meta SET value=1 WHERE key='tier'", [])?;
        bump_generation(&transaction)?;
        transaction.commit()?;
        self.generation()
    }

    pub fn counts(&self) -> Result<CacheCounts, CacheError> {
        Ok(self.db.query_row("SELECT (SELECT count(*) FROM files),(SELECT count(*) FROM symbols),(SELECT count(*) FROM imports),(SELECT count(*) FROM test_ranges),(SELECT count(*) FROM content_fts)", [], |row| Ok(CacheCounts { files:unsigned(row,0)?,symbols:unsigned(row,1)?,imports:unsigned(row,2)?,test_ranges:unsigned(row,3)?,content:unsigned(row,4)? }))?)
    }
}

/// Read a column SQLite stores as a signed integer into the unsigned domain the
/// cache promises, refusing negative values the way rusqlite once did for `u64`.
fn unsigned(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn bounded(limit: usize) -> i64 {
    limit.min(MAX_ROWS) as i64
}

fn validate_query(query: &str) -> Result<(), CacheError> {
    if query.len() > MAX_QUERY_BYTES || query.contains('\0') {
        return Err(CacheError::InvalidInput(
            "query exceeds 1024 bytes or contains NUL".to_owned(),
        ));
    }
    Ok(())
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<IndexedFile> {
    Ok(IndexedFile {
        path: row.get(0)?,
        language: row.get(1)?,
        size: unsigned(row, 2)?,
        mtime_ms: row.get(3)?,
        mode: row.get(4)?,
        content_hash: row.get(5)?,
        junk: row.get(6)?,
        content: None,
        symbols: Vec::new(),
        imports: Vec::new(),
        test_ranges: Vec::new(),
    })
}

fn symbol_row(row: &Row<'_>, start: usize) -> rusqlite::Result<WorkerSymbol> {
    Ok(WorkerSymbol {
        name: row.get(start)?,
        kind: row.get(start + 1)?,
        start_line: row.get(start + 2)?,
        start_col: row.get(start + 3)?,
        end_line: row.get(start + 4)?,
        detail: row.get(start + 5)?,
        doc: row.get(start + 6)?,
    })
}

fn delete_file(db: &Connection, path: &str) -> rusqlite::Result<()> {
    // UNINDEXED path lookup in FTS scans the entire index; rowid deletion
    // preserves constant per-file mutation work and prevents rowid reuse leaks.
    db.execute(
        "DELETE FROM content_fts WHERE rowid=(SELECT rowid FROM files WHERE path=?)",
        [path],
    )?;
    db.execute("DELETE FROM symbols WHERE path=?", [path])?;
    db.execute("DELETE FROM imports WHERE path=?", [path])?;
    db.execute("DELETE FROM test_ranges WHERE path=?", [path])?;
    db.execute("DELETE FROM files WHERE path=?", [path])?;
    Ok(())
}

fn bump_generation(db: &Connection) -> rusqlite::Result<()> {
    db.execute("UPDATE meta SET value=value+1 WHERE key='generation'", [])?;
    Ok(())
}

fn cache_paths(path: &Path) -> Vec<PathBuf> {
    ["", "-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| {
            let mut name = path.as_os_str().to_owned();
            name.push(suffix);
            PathBuf::from(name)
        })
        .collect()
}

fn lock_native_cache(path: &Path) -> Result<CacheOwner, CacheError> {
    check_cache_path(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let owner = options.open(path)?;
    // Verify the opened object too, before locking; a raced path replacement
    // must not acquire an ownership lock on an unrelated file.
    let metadata = owner.metadata()?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(CacheError::InvalidInput(
            "native lock is not a regular file".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(CacheError::InvalidInput(
                "native lock has hard links".to_owned(),
            ));
        }
    }
    #[cfg(windows)]
    check_windows_link_count(&owner, path)?;
    check_cache_path(path)?;
    crate::os_file_lock::try_lock_exclusive(&owner)?;
    Ok(CacheOwner(owner))
}

fn check_cache_path(path: &Path) -> Result<(), CacheError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(CacheError::InvalidInput(format!(
            "cache path is not a regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(CacheError::InvalidInput(format!(
                "cache path has hard links: {}",
                path.display()
            )));
        }
    }
    #[cfg(windows)]
    {
        let file = std::fs::File::open(path)?;
        check_windows_link_count(&file, path)?;
    }
    Ok(())
}

#[cfg(windows)]
fn check_windows_link_count(file: &std::fs::File, path: &Path) -> Result<(), CacheError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the file keeps its handle open throughout the call, and
    // information points to correctly sized writable storage.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: a successful GetFileInformationByHandle initialized this value.
    if unsafe { information.assume_init() }.nNumberOfLinks != 1 {
        return Err(CacheError::InvalidInput(format!(
            "cache path has hard links: {}",
            path.display()
        )));
    }
    Ok(())
}

fn is_corruption(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code, rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase))
}

fn probe(path: &Path) -> Result<Option<String>, CacheError> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    db.busy_timeout(Duration::from_millis(100))?;
    let integrity: String = db.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Ok(Some(format!(
            "native cache quick_check failed: {integrity}"
        )));
    }
    let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != VERSION {
        return Ok(Some(format!(
            "native cache schema version {version}, expected {VERSION}"
        )));
    }
    for (name, expected) in SCHEMA {
        let sql: Option<String> = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name=?",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        if sql.as_deref() != Some(*expected) {
            return Ok(Some(format!("native cache schema mismatch: {name}")));
        }
    }
    let triggers: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='trigger'",
        [],
        |row| row.get(0),
    )?;
    if triggers != 0 {
        return Ok(Some("native cache contains unexpected triggers".to_owned()));
    }
    let metadata: i64 = db.query_row("SELECT count(*) FROM meta WHERE (key='tier' AND value IN (0,1)) OR (key='generation' AND typeof(value)='integer' AND value>=0)", [], |row| row.get(0))?;
    if metadata != 2 {
        return Ok(Some(
            "native cache lifecycle metadata is invalid".to_owned(),
        ));
    }
    Ok(None)
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
