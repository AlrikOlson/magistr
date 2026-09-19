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
use super::session_record::SessionRecord;
use super::session_turn::{SessionTurn, TurnRole};

/// Database filename.
const DB_FILENAME: &str = "sessions.db";

/// Current schema version (for future migrations).
///
/// Must stay in lock-step with the GUI's copy of these migrations
/// (`crates/magistr-gui/src-tauri/src/session_tracker.rs`) — both binaries
/// migrate the same shared `sessions.db`.
const SCHEMA_VERSION: i32 = 6;

/// The session database, `<data_dir>/sessions.db`, over the captured data
/// directory — the same directory the cost ledger and the audit log share;
/// `None` when this machine has nowhere to keep one.
#[must_use]
pub fn sessions_db_path(inputs: &ProjectInputs) -> Option<PathBuf> {
    inputs.data_dir().map(|dir| dir.join(DB_FILENAME))
}

/// Open (or create) the session database at the given path.
///
/// Creates parent directories and initializes the schema if needed.
pub fn open_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
    }

    let conn =
        Connection::open(path).map_err(|e| MagistrError::sqlite("open session database", e))?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| MagistrError::sqlite("set WAL mode", e))?;

    migrate(&conn)?;
    Ok(conn)
}

/// Run schema migrations based on PRAGMA user_version.
fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| MagistrError::sqlite("read schema version", e))?;

    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id    TEXT PRIMARY KEY,
                project       TEXT NOT NULL,
                started_at    TEXT NOT NULL,
                ended_at      TEXT NOT NULL,
                cost_usd      REAL NOT NULL,
                turns         INTEGER NOT NULL,
                duration_ms   INTEGER NOT NULL,
                files_changed TEXT NOT NULL DEFAULT '[]',
                tags          TEXT NOT NULL DEFAULT '[]',
                summary       TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project);
            CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);

            PRAGMA user_version = 1;",
        )
        .map_err(|e| MagistrError::sqlite("create sessions table", e))?;
    }

    if version < 2 {
        conn.execute_batch(
            "ALTER TABLE sessions ADD COLUMN model TEXT NOT NULL DEFAULT '';
             PRAGMA user_version = 2;",
        )
        .map_err(|e| MagistrError::sqlite("migrate sessions v2 (add model column)", e))?;
    }

    if version < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_turns (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id    TEXT NOT NULL,
                turn_index    INTEGER NOT NULL,
                role          TEXT NOT NULL,
                content       TEXT NOT NULL,
                tool_name     TEXT,
                tool_id       TEXT,
                timestamp     TEXT NOT NULL,
                UNIQUE(session_id, turn_index)
            );

            CREATE INDEX IF NOT EXISTS idx_turns_session ON session_turns(session_id);

            PRAGMA user_version = 3;",
        )
        .map_err(|e| MagistrError::sqlite("migrate sessions v3 (add session_turns table)", e))?;
    }

    // v4 and v5 originated in the GUI's session tracker; mirrored here so a
    // CLI-first database carries the same columns the GUI's INSERT names.
    if version < 4 {
        conn.execute_batch(
            "ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'agent';
             CREATE INDEX IF NOT EXISTS idx_sessions_kind ON sessions(kind);
             PRAGMA user_version = 4;",
        )
        .map_err(|e| MagistrError::sqlite("migrate sessions v4 (add kind column)", e))?;
    }

    if version < 5 {
        conn.execute_batch(
            "ALTER TABLE sessions ADD COLUMN input_tokens INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE sessions ADD COLUMN output_tokens INTEGER NOT NULL DEFAULT 0;
             PRAGMA user_version = 5;",
        )
        .map_err(|e| MagistrError::sqlite("migrate sessions v5 (add token columns)", e))?;
    }

    if version < 6 {
        conn.execute_batch(
            "ALTER TABLE sessions ADD COLUMN owner TEXT;
             PRAGMA user_version = 6;",
        )
        .map_err(|e| MagistrError::sqlite("migrate sessions v6 (add owner column)", e))?;
    }

    let _ = SCHEMA_VERSION; // suppress unused warning

    Ok(())
}

/// Insert a session record into the database.
///
/// If a record with the same session_id already exists, it is replaced.
pub fn insert_session(conn: &Connection, record: &SessionRecord) -> Result<()> {
    let files_json = serde_json::to_string(&record.files_changed)
        .map_err(|e| MagistrError::parse("JSON", "serialize session files", e))?;
    let tags_json = serde_json::to_string(&record.tags)
        .map_err(|e| MagistrError::parse("JSON", "serialize session tags", e))?;

    conn.execute(
        "INSERT OR REPLACE INTO sessions
            (session_id, project, started_at, ended_at, cost_usd, turns,
             duration_ms, files_changed, tags, summary, model, owner)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            record.session_id,
            record.project,
            record.started_at.to_rfc3339(),
            record.ended_at.to_rfc3339(),
            record.cost_usd,
            record.turns,
            record.duration_ms as i64,
            files_json,
            tags_json,
            record.summary,
            record.model,
            record.owner,
        ],
    )
    .map_err(|e| MagistrError::sqlite("insert session", e))?;

    Ok(())
}

/// Query sessions with optional filters.
///
/// Returns results sorted by started_at descending (most recent first).
pub fn query_sessions(conn: &Connection, filter: &SessionFilter) -> Result<Vec<SessionRecord>> {
    let mut sql = String::from(
        "SELECT session_id, project, started_at, ended_at, cost_usd, turns,
                duration_ms, files_changed, tags, summary, model, owner
         FROM sessions WHERE 1=1",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref project) = filter.project {
        sql.push_str(" AND project LIKE ?");
        param_values.push(Box::new(format!("%{project}%")));
    }

    if let Some(since) = filter.since {
        sql.push_str(" AND started_at >= ?");
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

    sql.push_str(" ORDER BY started_at DESC LIMIT ?");
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
            "SELECT session_id, project, started_at, ended_at, cost_usd, turns,
                    duration_ms, files_changed, tags, summary, model, owner
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
            "DELETE FROM sessions WHERE started_at < ?1",
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

/// Convert a database row to a SessionTurn.
fn row_to_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionTurn> {
    let turn_index: i64 = row.get(0)?;
    let role_str: String = row.get(1)?;
    let timestamp_str: String = row.get(5)?;

    let role = TurnRole::parse_role(&role_str).unwrap_or(TurnRole::Assistant);
    let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(SessionTurn {
        turn_index: turn_index as u32,
        role,
        content: row.get(2)?,
        tool_name: row.get(3)?,
        tool_id: row.get(4)?,
        timestamp,
    })
}

/// Convert a database row to a SessionRecord.
fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    let started_str: String = row.get(2)?;
    let ended_str: String = row.get(3)?;
    let files_json: String = row.get(7)?;
    let tags_json: String = row.get(8)?;
    let duration_ms: i64 = row.get(6)?;

    let started_at = DateTime::parse_from_rfc3339(&started_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let ended_at = DateTime::parse_from_rfc3339(&ended_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let files_changed: Vec<String> = serde_json::from_str(&files_json).unwrap_or_default();
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    let model: String = row.get(10).unwrap_or_default();
    let owner: Option<String> = row.get(11).unwrap_or_default();

    Ok(SessionRecord {
        session_id: row.get(0)?,
        project: row.get(1)?,
        started_at,
        ended_at,
        cost_usd: row.get(4)?,
        turns: row.get(5)?,
        duration_ms: duration_ms as u64,
        files_changed,
        tags,
        summary: row.get(9)?,
        model,
        owner,
    })
}

#[cfg(test)]
#[path = "storage.tests.rs"]
mod tests;
