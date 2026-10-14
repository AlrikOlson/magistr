//! SQLite session storage.
//!
//! Stores session records in `~/.local/share/magistr/sessions.db`.
//! Uses `rusqlite` with the `bundled` feature for zero system dependencies.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, params};

use crate::config::inputs::ProjectInputs;
use crate::error::{MagistrError, Result};

use super::retention_policy::RetentionPolicy;
use super::session_filter::SessionFilter;
use super::session_outcome::SessionOutcome;
use super::session_record::SessionRecord;
use super::session_turn::{SessionTurn, TurnRole};

/// Database filename.
const DB_FILENAME: &str = "sessions.db";

/// How long an opener waits for another process's write lock.
// History writes must report contention within the host's close deadline.
const BUSY_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

/// Current schema only. Older formats are refused, never migrated.
const SCHEMA_VERSION: i32 = 9;

/// The session database, `<data_dir>/sessions.db`, over the captured data
/// directory — the same directory the cost ledger and the audit log share;
/// `None` when this machine has nowhere to keep one.
#[must_use]
pub fn sessions_db_path(inputs: &ProjectInputs) -> Option<PathBuf> {
    inputs.data_dir().map(|dir| dir.join(DB_FILENAME))
}

/// Open existing session history without creating or updating its schema.
pub fn open_readonly_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| MagistrError::sqlite("read session history", error))?;
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| MagistrError::sqlite("read saved conversations", error))?;
    if version != SCHEMA_VERSION {
        return Err(MagistrError::Config(
            "Saved conversations in this folder cannot be read by this version of Magistr.".into(),
        ));
    }
    Ok(conn)
}
/// Open or create the session database, including its parent directories and schema.
pub fn open_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
    }

    let conn =
        Connection::open(path).map_err(|e| MagistrError::sqlite("open session database", e))?;
    // Several Magistr processes share this file. One that finds it busy waits
    // its turn instead of failing: switching the journal mode and upgrading
    // the schema both need the write lock.
    conn.busy_timeout(BUSY_WAIT)
        .map_err(|e| MagistrError::sqlite("set the session database's busy wait", e))?;

    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| MagistrError::sqlite("enable session relationships", e))?;
    write_ahead(&conn)?;
    initialize(&conn)?;
    Ok(conn)
}

/// Write-ahead journaling, for readers beside a writer. The mode is a property
/// of the file, so an opener that finds it set has nothing to do. Switching it
/// needs the file to itself and SQLite does not apply the busy wait to that
/// switch: two openers of a new file collide, and the loser tries again until
/// it finds the winner's work done.
fn write_ahead(conn: &Connection) -> Result<()> {
    let deadline = std::time::Instant::now() + BUSY_WAIT;
    loop {
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .map_err(|e| MagistrError::sqlite("read the journal mode", e))?;
        if mode.eq_ignore_ascii_case("wal") {
            return Ok(());
        }
        match conn.execute_batch("PRAGMA journal_mode=WAL;") {
            Ok(()) => return Ok(()),
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if matches!(
                    failure.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(e) => return Err(MagistrError::sqlite("set WAL mode", e)),
        }
    }
}

/// Create the current schema atomically, or refuse an unsupported format.
fn initialize(conn: &Connection) -> Result<()> {
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| MagistrError::sqlite("read session schema", e))?;
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| MagistrError::sqlite("lock the session database for its schema", e))?;
    let result = (|| {
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| MagistrError::sqlite("read session schema", e))?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        if version != 0 {
            return Err(MagistrError::Config(
                "The saved session database has an unsupported format. Choose an empty data directory.".into(),
            ));
        }
        conn.execute_batch(
            "CREATE TABLE sessions (
                session_id TEXT PRIMARY KEY,
                project TEXT NOT NULL,
                workspace TEXT NOT NULL,
                provider TEXT NOT NULL,
                provider_session_id TEXT,
                resume_supported INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                outcome TEXT CHECK(outcome IN ('completed','cancelled','failed','stopped')),
                cost_usd REAL,
                turns INTEGER,
                duration_ms INTEGER NOT NULL,
                files_changed TEXT NOT NULL,
                tags TEXT NOT NULL,
                summary TEXT,
                model TEXT,
                owner TEXT,
                name TEXT,
                ended_at TEXT,
                CHECK ((ended_at IS NULL) = (outcome IS NULL))
            );
            CREATE TABLE session_drafts (
                session_id TEXT PRIMARY KEY REFERENCES sessions(session_id) ON DELETE CASCADE,
                revision INTEGER NOT NULL CHECK(revision >= 0),
                text TEXT NOT NULL,
                attachments BLOB NOT NULL
            );
            CREATE INDEX idx_sessions_project ON sessions(project);
            CREATE INDEX idx_sessions_workspace ON sessions(workspace);
            CREATE INDEX idx_sessions_started ON sessions(started_at);
            CREATE TABLE session_turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_name TEXT,
                tool_id TEXT,
                timestamp TEXT NOT NULL,
                UNIQUE(session_id, turn_index)
            );
            CREATE INDEX idx_turns_session ON session_turns(session_id);
            PRAGMA user_version = 9;",
        )
        .map_err(|e| MagistrError::sqlite("create session schema", e))
    })();
    match result {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|e| MagistrError::sqlite("commit session schema", e)),
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(error)
        }
    }
}

/// Save current owner facts. A repeated or older checkpoint is a no-op.
/// User-assigned tags and ownership survive a later continuation.
pub fn insert_session(conn: &Connection, record: &SessionRecord) -> Result<()> {
    let files = serde_json::to_string(&record.files_changed)
        .map_err(|e| MagistrError::parse("JSON", "serialize session files", e))?;
    let tags = serde_json::to_string(&record.tags)
        .map_err(|e| MagistrError::parse("JSON", "serialize session tags", e))?;
    let updated = conn
        .execute(
            "INSERT INTO sessions
            (session_id, project, started_at, updated_at, cost_usd, turns,
             duration_ms, files_changed, tags, summary, model, owner,
             workspace, provider, provider_session_id, resume_supported, outcome, name, ended_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15, ?16, ?17, ?18, ?19)
         ON CONFLICT(session_id) DO UPDATE SET
             updated_at=excluded.updated_at, cost_usd=excluded.cost_usd,
             turns=excluded.turns, duration_ms=excluded.duration_ms,
             files_changed=excluded.files_changed, summary=excluded.summary,
             model=excluded.model, provider_session_id=excluded.provider_session_id,
             resume_supported=excluded.resume_supported, outcome=excluded.outcome, name=excluded.name,
             ended_at=excluded.ended_at
         WHERE excluded.updated_at > sessions.updated_at
           AND excluded.project = sessions.project
           AND excluded.workspace = sessions.workspace
           AND excluded.provider = sessions.provider",
            params![
                record.session_id,
                record.project,
                record.started_at.to_rfc3339(),
                record.updated_at.to_rfc3339(),
                record.cost_usd,
                record.turns,
                i64::try_from(record.duration_ms)
                    .map_err(|e| MagistrError::sqlite("session duration", e))?,
                files,
                tags,
                record.summary,
                record.model,
                record.owner,
                record.workspace,
                record.provider,
                record.provider_session_id,
                record.resume_supported,
                record.outcome.map(SessionOutcome::as_str),
                record.name,
                record.ended_at.map(|time| time.to_rfc3339()),
            ],
        )
        .map_err(|e| MagistrError::sqlite("save session", e))?;
    if updated == 0
        && let Some(existing) = get_session(conn, &record.session_id)?
        && (existing.project != record.project
            || existing.workspace != record.workspace
            || existing.provider != record.provider)
    {
        return Err(MagistrError::Config(
            "A saved conversation cannot be reassigned to another project or agent.".into(),
        ));
    }
    Ok(())
}

/// Query sessions with optional filters.
///
/// Returns results sorted by the latest checkpoint, including open conversations.
pub fn query_sessions(conn: &Connection, filter: &SessionFilter) -> Result<Vec<SessionRecord>> {
    let mut sql = String::from(
        "SELECT session_id, project, started_at, updated_at, cost_usd, turns,
                duration_ms, files_changed, tags, summary, model, owner,
                workspace, provider, provider_session_id, resume_supported, outcome, name, ended_at
         FROM sessions WHERE 1=1",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref project) = filter.project {
        sql.push_str(" AND project LIKE ?");
        param_values.push(Box::new(format!("%{project}%")));
    }

    if let Some(workspace) = &filter.workspace {
        sql.push_str(" AND workspace = ?");
        param_values.push(Box::new(workspace.clone()));
    }

    if let Some(since) = filter.since {
        sql.push_str(" AND updated_at >= ?");
        param_values.push(Box::new(since.to_rfc3339()));
    }

    if let Some(ref tag) = filter.tag {
        // JSON array contains check: tags field contains the tag string
        sql.push_str(" AND tags LIKE ?");
        param_values.push(Box::new(format!("%\"{tag}\"%")));
    }

    if let Some(ref owner) = filter.owner {
        sql.push_str(" AND owner LIKE ?");
        param_values.push(Box::new(format!("%{owner}%")));
    }

    sql.push_str(" ORDER BY updated_at DESC, session_id LIMIT ?");
    param_values.push(Box::new(filter.limit as i64));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| MagistrError::sqlite("prepare sessions query", e))?;

    let rows = stmt
        .query_map(params_ref.as_slice(), row_to_record)
        .map_err(|e| MagistrError::sqlite("query sessions", e))?;

    let mut records = Vec::new();
    for row in rows {
        let record = row.map_err(|e| MagistrError::sqlite("read session row", e))?;
        records.push(record);
    }

    Ok(records)
}

/// Get a single session by ID.
pub fn get_session(conn: &Connection, session_id: &str) -> Result<Option<SessionRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT session_id, project, started_at, updated_at, cost_usd, turns,
                    duration_ms, files_changed, tags, summary, model, owner,
                workspace, provider, provider_session_id, resume_supported, outcome, name, ended_at
             FROM sessions WHERE session_id = ?1",
        )
        .map_err(|e| MagistrError::sqlite("prepare session query", e))?;

    let mut rows = stmt
        .query_map(params![session_id], row_to_record)
        .map_err(|e| MagistrError::sqlite("query session by id", e))?;

    match rows.next() {
        Some(row) => {
            let record = row.map_err(|e| MagistrError::sqlite("read session row", e))?;
            Ok(Some(record))
        }
        None => Ok(None),
    }
}

/// Delete sessions older than the retention policy.
///
/// Returns the number of sessions purged.
pub fn purge_old_sessions(conn: &Connection, policy: &RetentionPolicy) -> Result<u32> {
    let cutoff = Utc::now() - Duration::days(i64::from(policy.max_age_days));

    let deleted = conn
        .execute(
            "DELETE FROM sessions WHERE updated_at < ?1",
            params![cutoff.to_rfc3339()],
        )
        .map_err(|e| MagistrError::sqlite("purge old sessions", e))?;

    Ok(deleted as u32)
}

/// Count total sessions in the database.
pub fn count_sessions(conn: &Connection) -> Result<u64> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .map_err(|e| MagistrError::sqlite("count sessions", e))?;

    Ok(count as u64)
}

/// Add a tag to an existing session.
pub fn tag_session(conn: &Connection, session_id: &str, tag: &str) -> Result<()> {
    let record = get_session(conn, session_id)?.ok_or_else(|| {
        MagistrError::sqlite("tag session", format!("session {session_id} not found"))
    })?;

    let mut tags = record.tags;
    if !tags.iter().any(|t| t == tag) {
        tags.push(tag.to_string());
    }

    let tags_json = serde_json::to_string(&tags)
        .map_err(|e| MagistrError::parse("JSON", "serialize session tags", e))?;

    conn.execute(
        "UPDATE sessions SET tags = ?1 WHERE session_id = ?2",
        params![tags_json, session_id],
    )
    .map_err(|e| MagistrError::sqlite("update session tags", e))?;

    Ok(())
}

/// Transfer a session to a new owner.
///
/// Errors when the session does not exist. Authorization is the caller's
/// job — see `team::session_access`.
pub fn set_session_owner(conn: &Connection, session_id: &str, new_owner: &str) -> Result<()> {
    let updated = conn
        .execute(
            "UPDATE sessions SET owner = ?1 WHERE session_id = ?2",
            params![new_owner, session_id],
        )
        .map_err(|e| MagistrError::sqlite("set session owner", e))?;

    if updated == 0 {
        return Err(MagistrError::sqlite(
            "set session owner",
            format!("session {session_id} not found"),
        ));
    }

    Ok(())
}

// ─── Session Turn operations ────────────────────────────────────────────────

/// Insert a conversation turn for a session.
pub fn insert_turn(conn: &Connection, session_id: &str, turn: &SessionTurn) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO session_turns
            (session_id, turn_index, role, content, tool_name, tool_id, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            session_id,
            turn.turn_index,
            turn.role.as_str(),
            turn.content,
            turn.tool_name,
            turn.tool_id,
            turn.timestamp.to_rfc3339(),
        ],
    )
    .map_err(|e| MagistrError::sqlite("insert session turn", e))?;

    Ok(())
}

/// Query all turns for a session, ordered by turn_index ascending.
pub fn query_turns(conn: &Connection, session_id: &str) -> Result<Vec<SessionTurn>> {
    let mut stmt = conn
        .prepare(
            "SELECT turn_index, role, content, tool_name, tool_id, timestamp
             FROM session_turns
             WHERE session_id = ?1
             ORDER BY turn_index ASC",
        )
        .map_err(|e| MagistrError::sqlite("prepare turns query", e))?;

    let rows = stmt
        .query_map(params![session_id], row_to_turn)
        .map_err(|e| MagistrError::sqlite("query session turns", e))?;

    let mut turns = Vec::new();
    for row in rows {
        let turn = row.map_err(|e| MagistrError::sqlite("read turn row", e))?;
        turns.push(turn);
    }

    Ok(turns)
}

/// Delete all turns for a session (used when purging sessions).
pub fn delete_turns(conn: &Connection, session_id: &str) -> Result<u32> {
    let deleted = conn
        .execute(
            "DELETE FROM session_turns WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(|e| MagistrError::sqlite("delete session turns", e))?;

    Ok(deleted as u32)
}

/// Count turns for a session.
pub fn count_turns(conn: &Connection, session_id: &str) -> Result<u32> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM session_turns WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .map_err(|e| MagistrError::sqlite("count session turns", e))?;

    Ok(count as u32)
}

fn decode<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    column: usize,
) -> rusqlite::Result<T> {
    let text: String = row.get(column)?;
    serde_json::from_str(&text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn timestamp(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<DateTime<Utc>> {
    let text: String = row.get(column)?;
    DateTime::parse_from_rfc3339(&text)
        .map(|date| date.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn row_to_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionTurn> {
    let role: String = row.get(1)?;
    let role = TurnRole::parse_role(&role).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other("Unknown conversation role")),
        )
    })?;
    Ok(SessionTurn {
        turn_index: row.get(0)?,
        role,
        content: row.get(2)?,
        tool_name: row.get(3)?,
        tool_id: row.get(4)?,
        timestamp: timestamp(row, 5)?,
    })
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    let outcome: Option<String> = row.get(16)?;
    let outcome = outcome
        .map(|value| {
            serde_json::from_value::<SessionOutcome>(serde_json::Value::String(value)).map_err(
                |error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        16,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                },
            )
        })
        .transpose()?;
    Ok(SessionRecord {
        session_id: row.get(0)?,
        name: row.get(17)?,
        project: row.get(1)?,
        started_at: timestamp(row, 2)?,
        updated_at: timestamp(row, 3)?,
        ended_at: row
            .get::<_, Option<String>>(18)?
            .map(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|time| time.with_timezone(&Utc))
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            18,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
            })
            .transpose()?,
        cost_usd: row.get(4)?,
        turns: row.get(5)?,
        duration_ms: u64::try_from(row.get::<_, i64>(6)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?,
        files_changed: decode(row, 7)?,
        tags: decode(row, 8)?,
        summary: row.get(9)?,
        model: row.get(10)?,
        owner: row.get(11)?,
        workspace: row.get(12)?,
        provider: row.get(13)?,
        provider_session_id: row.get(14)?,
        resume_supported: row.get(15)?,
        outcome,
    })
}

#[cfg(test)]
#[path = "storage.tests.rs"]
mod tests;
