use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::{JournalDbOptions, JournalError};

const VERSION: i64 = 3;
const SUFFIXES: [&str; 4] = ["", "-wal", "-shm", "-journal"];
const SCHEMA: [(&str, &str); 5] = [
    (
        "ops",
        "CREATE TABLE ops(seq INTEGER PRIMARY KEY AUTOINCREMENT,at INTEGER NOT NULL,tool TEXT NOT NULL,args_hash TEXT NOT NULL,session_id TEXT NOT NULL,is_undo INTEGER NOT NULL DEFAULT 0)",
    ),
    (
        "ops_session",
        "CREATE INDEX ops_session ON ops(session_id, seq)",
    ),
    (
        "op_files",
        "CREATE TABLE op_files(op_seq INTEGER NOT NULL REFERENCES ops(seq),ord INTEGER NOT NULL,kind TEXT NOT NULL CHECK(kind IN ('create','write','delete')),path TEXT NOT NULL,before_hash TEXT NOT NULL,after_hash TEXT NOT NULL,PRIMARY KEY(op_seq, ord))",
    ),
    (
        "blobs",
        "CREATE TABLE blobs(hash TEXT PRIMARY KEY,zstd BLOB NOT NULL,bytes INTEGER NOT NULL,last_ref_seq INTEGER NOT NULL)",
    ),
    (
        "undone",
        "CREATE TABLE undone(op_seq INTEGER PRIMARY KEY,by_seq INTEGER NOT NULL)",
    ),
];

/// Owns the result of the durable lifecycle decision, shared by writer and reader.
pub(super) struct OpenedJournal {
    pub(super) db: Connection,
    pub(super) notes: Vec<String>,
    pub(super) quarantined_to: Option<PathBuf>,
}

impl OpenedJournal {
    pub(super) fn writer(options: &JournalDbOptions) -> Result<Self, JournalError> {
        if options.cap_bytes > i64::MAX as u64 {
            return Err(JournalError::InvalidInput(
                "cap exceeds SQLite integer range".to_owned(),
            ));
        }
        let path = normalized_path(&options.path, true)?;
        check_paths(&path)?;
        reject_orphaned_sidecars(&path)?;
        let mut quarantined_to = None;
        let mut notes = Vec::new();
        let probe = if path.try_exists()? {
            match probe(&path, false) {
                Ok(found) => Some(found),
                Err(JournalError::Sqlite(error)) if is_corruption(&error) => {
                    let target = quarantine(&path)?;
                    notes.push(format!(
                        "{}: unreadable durable database quarantined intact as {}; a fresh database was created",
                        name(&path), name(&target)
                    ));
                    quarantined_to = Some(target);
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let db = finish_open(&path, probe, options).map_err(|error| match &quarantined_to {
            Some(path) => JournalError::RecoveryFailed {
                quarantined_to: path.clone(),
                source: Box::new(error),
            },
            None => error,
        })?;
        Ok(Self {
            db,
            notes,
            quarantined_to,
        })
    }

    pub(super) fn reader(path: &Path) -> Result<Connection, JournalError> {
        let path = normalized_path(path, false)?;
        check_paths(&path)?;
        let db = open(&path, true)?;
        validate(&db, true)?;
        Ok(db)
    }
}

fn finish_open(
    path: &Path,
    probe: Option<(i64, bool)>,
    options: &JournalDbOptions,
) -> Result<Connection, JournalError> {
    let mut db = open(path, false)?;
    db.query_row(
        &format!("PRAGMA journal_mode={}", options.journal_mode.sql()),
        [],
        |_| Ok(()),
    )?;
    if probe.is_none_or(|(_, empty)| empty) {
        let transaction = db.transaction()?;
        for (_, sql) in SCHEMA {
            transaction.execute_batch(sql)?;
        }
        transaction.execute_batch("PRAGMA user_version=2")?;
        transaction.commit()?;
    } else if probe.is_some_and(|(version, _)| version == 1) {
        // v1 already stores exact UTF-8 bytes in the same CAS layout. Only
        // the interpretation broadens; never recompress or rewrite a row.
        let transaction = db.transaction()?;
        transaction.execute_batch("PRAGMA user_version=2")?;
        transaction.commit()?;
    }
    Ok(db)
}

fn normalized_path(path: &Path, create_parent: bool) -> Result<PathBuf, JournalError> {
    let filename = path
        .file_name()
        .ok_or_else(|| JournalError::InvalidInput("journal path must name a file".to_owned()))?;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if create_parent {
        std::fs::create_dir_all(parent)?;
    }
    Ok(parent.canonicalize()?.join(filename))
}

fn open(path: &Path, readonly: bool) -> Result<Connection, JournalError> {
    let access = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let db = Connection::open_with_flags(
        path,
        access | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    db.busy_timeout(Duration::from_millis(100))?;
    // Bound SQLite's own row allocation too, before Rust can inspect ValueRef.
    // The extra MiB covers the 64-MiB compressed blob plus its row metadata.
    db.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        65 * 1024 * 1024,
    )?;
    Ok(db)
}

fn probe(path: &Path, reader: bool) -> Result<(i64, bool), JournalError> {
    // A hot rollback sidecar can make a readonly SQLite connection request
    // recovery before inspecting the main header. Measure incontrovertible
    // main-file corruption without engaging SQLite's sidecar machinery.
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    let mut header = [0u8; 16];
    if length != 0
        && (length < 100 || {
            file.read_exact(&mut header)?;
            &header != b"SQLite format 3\0"
        })
    {
        return Err(JournalError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_NOTADB),
            Some("invalid or truncated SQLite header".to_owned()),
        )));
    }
    let db = open(path, true)?;
    validate(&db, reader)
}

fn validate(db: &Connection, reader: bool) -> Result<(i64, bool), JournalError> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    // Refuse newer headers before reading the schema or enabling any writable
    // connection, even when the database happens to contain no tables.
    if !(0..=VERSION).contains(&version) {
        return Err(JournalError::UnsupportedVersion(format!(
            "schema version {version}; this binary reads versions 1..={VERSION}"
        )));
    }
    let count: i64 = db.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
    if count == 0 && !reader && version < 3 {
        return Ok((version, true));
    }
    if !(1..=VERSION).contains(&version) {
        return Err(JournalError::UnsupportedVersion(format!(
            "schema version {version} has no registered forward migration"
        )));
    }
    for (name, expected) in SCHEMA.into_iter().chain(
        super::intent_store::IntentStore::SCHEMA
            .into_iter()
            .filter(|_| version == 3),
    ) {
        let sql: Option<String> = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name=?",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        if sql.is_none_or(|sql| normalized_sql(&sql) != normalized_sql(expected)) {
            return Err(JournalError::UnsupportedVersion(format!(
                "unrecognized journal schema for {name}; durable data left untouched"
            )));
        }
    }
    let unexpected: i64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE name NOT IN ('ops','ops_session','op_files','blobs','undone','sqlite_sequence','sqlite_autoindex_op_files_1','sqlite_autoindex_blobs_1') AND (?!=3 OR name NOT IN ('native_write_intents','native_intent_undo','sqlite_autoindex_native_intent_undo_1'))", [version], |row| row.get(0))?;
    if unexpected != 0 {
        return Err(JournalError::UnsupportedVersion(
            "journal contains unrecognized schema objects".to_owned(),
        ));
    }
    // The probe measures structural corruption, but never mistakes permission,
    // busy, disk, I/O, or schema incompatibility failures for corrupt data.
    let integrity: String = db.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(JournalError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some(integrity),
        )));
    }
    Ok((version, count == 0))
}

fn normalized_sql(sql: &str) -> String {
    let mut normalized = String::new();
    let mut quote = None;
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(delimiter) = quote {
            normalized.push(ch);
            if ch == delimiter {
                if chars.peek() == Some(&delimiter) {
                    normalized.push(chars.next().expect("peeked escaped quote"));
                } else {
                    quote = None;
                }
            }
        } else if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            normalized.push(ch);
        } else if !ch.is_whitespace() {
            normalized.extend(ch.to_lowercase());
        }
    }
    normalized
}

fn is_corruption(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code, rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase))
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn check_paths(path: &Path) -> Result<(), JournalError> {
    for suffix in SUFFIXES {
        let path = suffixed(path, suffix);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() || metadata.is_symlink() {
            return Err(JournalError::InvalidInput(format!(
                "journal path is not a regular file: {}",
                path.display()
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Err(JournalError::InvalidInput(format!(
                    "journal path has hard links: {}",
                    path.display()
                )));
            }
        }
        #[cfg(windows)]
        check_windows_link_count(&std::fs::File::open(&path)?, &path)?;
    }
    Ok(())
}

fn quarantine(path: &Path) -> Result<PathBuf, JournalError> {
    check_paths(path)?;
    let mut target = suffixed(path, ".quarantined");
    let mut counter = 2;
    while has_any_artifact(&target)? {
        target = suffixed(path, &format!(".quarantined.{counter}"));
        counter += 1;
    }
    let mut planned = Vec::new();
    for suffix in SUFFIXES {
        let source = suffixed(path, suffix);
        if artifact_exists(&source)? {
            planned.push((source, suffixed(&target, suffix)));
        }
    }
    let mut moved = Vec::new();
    for (source, destination) in planned {
        if let Err(error) = std::fs::rename(&source, &destination) {
            let mut rollback_errors = Vec::new();
            for (source, destination) in moved.into_iter().rev() {
                if let Err(rollback) = std::fs::rename(destination, source) {
                    rollback_errors.push(rollback.to_string());
                }
            }
            if !rollback_errors.is_empty() {
                return Err(std::io::Error::other(format!("quarantine transfer failed: {error}; rollback also failed: {}; durable artifacts remain at {} and {}", rollback_errors.join("; "), path.display(), target.display())).into());
            }
            return Err(error.into());
        }
        moved.push((source, destination));
    }
    Ok(target)
}

fn artifact_exists(path: &Path) -> Result<bool, JournalError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn has_any_artifact(path: &Path) -> Result<bool, JournalError> {
    for suffix in SUFFIXES {
        if artifact_exists(&suffixed(path, suffix))? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn reject_orphaned_sidecars(path: &Path) -> Result<(), JournalError> {
    let missing_or_empty = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len() == 0,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    if missing_or_empty {
        for suffix in &SUFFIXES[1..] {
            if artifact_exists(&suffixed(path, suffix))? {
                return Err(JournalError::UnsupportedVersion("missing or empty journal main file has durable sidecars; preserve and recover the artifact set before opening a writer".to_owned()));
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn check_windows_link_count(file: &std::fs::File, path: &Path) -> Result<(), JournalError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the file owns a live handle and information points to writable,
    // correctly sized storage for the duration of this call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: success above initialized the complete structure.
    if unsafe { information.assume_init() }.nNumberOfLinks != 1 {
        return Err(JournalError::InvalidInput(format!(
            "journal path has hard links: {}",
            path.display()
        )));
    }
    Ok(())
}

fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}
