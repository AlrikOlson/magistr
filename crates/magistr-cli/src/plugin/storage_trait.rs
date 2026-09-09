//! Storage backend plugin trait — contract for session persistence providers.
//!
//! Abstracts session storage behind a trait so that alternative backends
//! (PostgreSQL, S3) can replace the default SQLite store. The default
//! [`SqliteStorage`] wraps the existing `rusqlite` implementation.

use std::future::Future;
use std::pin::Pin;

use rusqlite::OptionalExtension;

use super::types::PluginError;

/// A session record in serialized form for storage plugin transport.
#[derive(Debug, Clone, PartialEq)]
pub struct StorageRecord {
    /// Unique session identifier.
    pub session_id: String,
    /// Project name or directory this session belongs to.
    pub project: Option<String>,
    /// Full record payload as JSON.
    pub data: serde_json::Value,
}

/// Filter criteria for querying session records.
#[derive(Debug, Clone, Default)]
pub struct StorageFilter {
    /// Filter by project name.
    pub project: Option<String>,
    /// Maximum number of records to return.
    pub limit: Option<usize>,
    /// Filter by session IDs.
    pub session_ids: Option<Vec<String>>,
}

/// Trait for session storage backends.
///
/// Implementations provide persistent storage for session records. The default
/// is SQLite (via `rusqlite`); alternatives include PostgreSQL and S3.
pub trait StoragePlugin: Send {
    /// Store a session record, creating or updating it.
    fn store(
        &mut self,
        record: StorageRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + '_>>;

    /// Retrieve a session record by session ID.
    fn get(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<StorageRecord>, PluginError>> + Send + '_>>;

    /// Query session records with a filter.
    fn query(
        &mut self,
        filter: StorageFilter,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<StorageRecord>, PluginError>> + Send + '_>>;

    /// Delete a session record by session ID.
    fn delete(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, PluginError>> + Send + '_>>;

    /// Get the storage backend name.
    fn name(&self) -> &str;
}

/// SQLite storage backend — wraps the existing `rusqlite`-based session store.
///
/// This is the default storage backend, providing zero-dependency local persistence.
pub struct SqliteStorage {
    /// Path to the SQLite database file.
    db_path: std::path::PathBuf,
}

impl SqliteStorage {
    /// Create a new SQLite storage backend at the given database path.
    pub fn new(db_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// Get the database file path.
    #[must_use]
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}

impl StoragePlugin for SqliteStorage {
    fn store(
        &mut self,
        record: StorageRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + '_>> {
        let db_path = self.db_path.clone();
        Box::pin(async move {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| PluginError::ProcessIo(format!("sqlite open: {e}")))?;

            conn.execute_batch("PRAGMA journal_mode=WAL;")
                .map_err(|e| PluginError::ProcessIo(format!("sqlite pragma: {e}")))?;

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS plugin_sessions (
                    session_id TEXT PRIMARY KEY,
                    project TEXT,
                    data TEXT NOT NULL
                )",
            )
            .map_err(|e| PluginError::ProcessIo(format!("sqlite create table: {e}")))?;

            let data_str = serde_json::to_string(&record.data)
                .map_err(|e| PluginError::ProcessIo(format!("serialize record: {e}")))?;

            conn.execute(
                "INSERT OR REPLACE INTO plugin_sessions (session_id, project, data) VALUES (?1, ?2, ?3)",
                rusqlite::params![record.session_id, record.project, data_str],
            )
            .map_err(|e| PluginError::ProcessIo(format!("sqlite insert: {e}")))?;

            Ok(())
        })
    }

    fn get(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<StorageRecord>, PluginError>> + Send + '_>> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| PluginError::ProcessIo(format!("sqlite open: {e}")))?;

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS plugin_sessions (
                    session_id TEXT PRIMARY KEY,
                    project TEXT,
                    data TEXT NOT NULL
                )",
            )
            .map_err(|e| PluginError::ProcessIo(format!("sqlite create table: {e}")))?;

            let mut stmt = conn
                .prepare(
                    "SELECT session_id, project, data FROM plugin_sessions WHERE session_id = ?1",
                )
                .map_err(|e| PluginError::ProcessIo(format!("sqlite prepare: {e}")))?;

            let result = stmt
                .query_row(rusqlite::params![session_id], |row| {
                    let sid: String = row.get(0)?;
                    let project: Option<String> = row.get(1)?;
                    let data_str: String = row.get(2)?;
                    Ok((sid, project, data_str))
                })
                .optional()
                .map_err(|e| PluginError::ProcessIo(format!("sqlite query: {e}")))?;

            match result {
                Some((sid, project, data_str)) => {
                    let data: serde_json::Value = serde_json::from_str(&data_str)
                        .map_err(|e| PluginError::ProcessIo(format!("deserialize record: {e}")))?;
                    Ok(Some(StorageRecord {
                        session_id: sid,
                        project,
                        data,
                    }))
                }
                None => Ok(None),
            }
        })
    }

    fn query(
        &mut self,
        filter: StorageFilter,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<StorageRecord>, PluginError>> + Send + '_>> {
        let db_path = self.db_path.clone();
        Box::pin(async move {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| PluginError::ProcessIo(format!("sqlite open: {e}")))?;

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS plugin_sessions (
                    session_id TEXT PRIMARY KEY,
                    project TEXT,
                    data TEXT NOT NULL
                )",
            )
            .map_err(|e| PluginError::ProcessIo(format!("sqlite create table: {e}")))?;

            let mut sql =
                String::from("SELECT session_id, project, data FROM plugin_sessions WHERE 1=1");
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref project) = filter.project {
                sql.push_str(" AND project = ?");
                param_values.push(Box::new(project.clone()));
            }

            if let Some(ref ids) = filter.session_ids
                && !ids.is_empty()
            {
                let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
                sql.push_str(&format!(" AND session_id IN ({})", placeholders.join(",")));
                for id in ids {
                    param_values.push(Box::new(id.clone()));
                }
            }

            let limit = filter.limit.unwrap_or(100);
            sql.push_str(" LIMIT ?");
            param_values.push(Box::new(limit as i64));

            let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| PluginError::ProcessIo(format!("sqlite prepare: {e}")))?;

            let rows = stmt
                .query_map(params_ref.as_slice(), |row| {
                    let sid: String = row.get(0)?;
                    let project: Option<String> = row.get(1)?;
                    let data_str: String = row.get(2)?;
                    Ok((sid, project, data_str))
                })
                .map_err(|e| PluginError::ProcessIo(format!("sqlite query: {e}")))?;

            let mut records = Vec::new();
            for row in rows {
                let (sid, project, data_str) =
                    row.map_err(|e| PluginError::ProcessIo(format!("sqlite row: {e}")))?;
                let data: serde_json::Value = serde_json::from_str(&data_str)
                    .map_err(|e| PluginError::ProcessIo(format!("deserialize record: {e}")))?;
                records.push(StorageRecord {
                    session_id: sid,
                    project,
                    data,
                });
            }

            Ok(records)
        })
    }

    fn delete(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, PluginError>> + Send + '_>> {
        let db_path = self.db_path.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| PluginError::ProcessIo(format!("sqlite open: {e}")))?;

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS plugin_sessions (
                    session_id TEXT PRIMARY KEY,
                    project TEXT,
                    data TEXT NOT NULL
                )",
            )
            .map_err(|e| PluginError::ProcessIo(format!("sqlite create table: {e}")))?;

            let deleted = conn
                .execute(
                    "DELETE FROM plugin_sessions WHERE session_id = ?1",
                    rusqlite::params![session_id],
                )
                .map_err(|e| PluginError::ProcessIo(format!("sqlite delete: {e}")))?;

            Ok(deleted > 0)
        })
    }

    fn name(&self) -> &str {
        "sqlite"
    }
}

/// PostgreSQL storage backend.
///
/// Connects to a PostgreSQL database via `DATABASE_URL` environment variable.
/// Stores session records in a `magistr_sessions` table.
pub struct PostgresStorage {
    /// Connection URL.
    database_url: String,
}

impl PostgresStorage {
    /// Create a new PostgreSQL storage backend.
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }

    /// Get the database URL (with credentials masked).
    #[must_use]
    pub fn masked_url(&self) -> String {
        if let Some(at_pos) = self.database_url.find('@')
            && let Some(scheme_end) = self.database_url.find("://")
        {
            return format!(
                "{}://***@{}",
                &self.database_url[..scheme_end],
                &self.database_url[at_pos + 1..]
            );
        }
        "postgres://***".to_string()
    }
}

impl StoragePlugin for PostgresStorage {
    fn store(
        &mut self,
        record: StorageRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + '_>> {
        let url = self.database_url.clone();
        Box::pin(async move {
            let data_str = serde_json::to_string(&record.data)
                .map_err(|e| PluginError::ProcessIo(format!("serialize record: {e}")))?;

            let body = serde_json::json!({
                "session_id": record.session_id,
                "project": record.project,
                "data": data_str,
            });

            ureq::post(&format!("{url}/magistr/sessions"))
                .content_type("application/json")
                .send(serde_json::to_string(&body).unwrap_or_default().as_bytes())
                .map_err(|e| PluginError::ProcessIo(format!("postgres store: {e}")))?;

            Ok(())
        })
    }

    fn get(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<StorageRecord>, PluginError>> + Send + '_>> {
        let url = self.database_url.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            let mut resp = ureq::get(&format!("{url}/magistr/sessions/{session_id}"))
                .call()
                .map_err(|e| PluginError::ProcessIo(format!("postgres get: {e}")))?;

            if resp.status() == 404 {
                return Ok(None);
            }

            let body_str: String = resp
                .body_mut()
                .read_to_string()
                .map_err(|e| PluginError::ProcessIo(format!("postgres read: {e}")))?;

            let body: serde_json::Value = serde_json::from_str(&body_str)
                .map_err(|e| PluginError::ProcessIo(format!("postgres parse: {e}")))?;

            Ok(Some(StorageRecord {
                session_id: body["session_id"].as_str().unwrap_or_default().to_string(),
                project: body["project"].as_str().map(String::from),
                data: body["data"].clone(),
            }))
        })
    }

    fn query(
        &mut self,
        filter: StorageFilter,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<StorageRecord>, PluginError>> + Send + '_>> {
        let url = self.database_url.clone();
        Box::pin(async move {
            let filter_json = serde_json::json!({
                "project": filter.project,
                "limit": filter.limit,
                "session_ids": filter.session_ids,
            });

            let mut resp = ureq::post(&format!("{url}/magistr/sessions/query"))
                .content_type("application/json")
                .send(
                    serde_json::to_string(&filter_json)
                        .unwrap_or_default()
                        .as_bytes(),
                )
                .map_err(|e| PluginError::ProcessIo(format!("postgres query: {e}")))?;

            let body_str: String = resp
                .body_mut()
                .read_to_string()
                .map_err(|e| PluginError::ProcessIo(format!("postgres read: {e}")))?;

            let body: Vec<serde_json::Value> = serde_json::from_str(&body_str)
                .map_err(|e| PluginError::ProcessIo(format!("postgres parse: {e}")))?;

            Ok(body
                .into_iter()
                .map(|v| StorageRecord {
                    session_id: v["session_id"].as_str().unwrap_or_default().to_string(),
                    project: v["project"].as_str().map(String::from),
                    data: v["data"].clone(),
                })
                .collect())
        })
    }

    fn delete(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, PluginError>> + Send + '_>> {
        let url = self.database_url.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            let resp = ureq::delete(&format!("{url}/magistr/sessions/{session_id}"))
                .call()
                .map_err(|e| PluginError::ProcessIo(format!("postgres delete: {e}")))?;

            Ok(resp.status() == 200)
        })
    }

    fn name(&self) -> &str {
        "postgresql"
    }
}

/// S3 storage backend.
///
/// Stores session records as JSON objects in an S3 bucket under a
/// `magistr/sessions/` prefix.
pub struct S3Storage {
    /// S3 bucket name.
    bucket: String,
    /// S3 endpoint URL (for S3-compatible services like MinIO).
    endpoint: Option<String>,
    /// Key prefix for session objects.
    prefix: String,
}

impl S3Storage {
    /// Create a new S3 storage backend.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            endpoint: None,
            prefix: "magistr/sessions".to_string(),
        }
    }

    /// Set a custom S3 endpoint (for MinIO, R2, etc.).
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Get the bucket name.
    #[must_use]
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Build the S3 URL for a session object.
    fn object_url(&self, session_id: &str) -> String {
        let base = self
            .endpoint
            .as_deref()
            .unwrap_or("https://s3.amazonaws.com");
        format!("{base}/{}/{}/{session_id}.json", self.bucket, self.prefix)
    }
}

impl StoragePlugin for S3Storage {
    fn store(
        &mut self,
        record: StorageRecord,
    ) -> Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + '_>> {
        let url = self.object_url(&record.session_id);
        Box::pin(async move {
            let body = serde_json::json!({
                "session_id": record.session_id,
                "project": record.project,
                "data": record.data,
            });

            ureq::put(&url)
                .content_type("application/json")
                .content_type("application/json")
                .send(serde_json::to_string(&body).unwrap_or_default().as_bytes())
                .map_err(|e| PluginError::ProcessIo(format!("s3 put: {e}")))?;

            Ok(())
        })
    }

    fn get(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<StorageRecord>, PluginError>> + Send + '_>> {
        let url = self.object_url(session_id);
        Box::pin(async move {
            let mut resp = match ureq::get(&url).call() {
                Ok(r) => r,
                Err(ureq::Error::StatusCode(404)) => return Ok(None),
                Err(e) => return Err(PluginError::ProcessIo(format!("s3 get: {e}"))),
            };

            let body_str: String = resp
                .body_mut()
                .read_to_string()
                .map_err(|e| PluginError::ProcessIo(format!("s3 read: {e}")))?;

            let body: serde_json::Value = serde_json::from_str(&body_str)
                .map_err(|e| PluginError::ProcessIo(format!("s3 parse: {e}")))?;

            Ok(Some(StorageRecord {
                session_id: body["session_id"].as_str().unwrap_or_default().to_string(),
                project: body["project"].as_str().map(String::from),
                data: body["data"].clone(),
            }))
        })
    }

    fn query(
        &mut self,
        _filter: StorageFilter,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<StorageRecord>, PluginError>> + Send + '_>> {
        // S3 listing requires ListObjectsV2 — basic implementation using prefix listing.
        let base = self
            .endpoint
            .as_deref()
            .unwrap_or("https://s3.amazonaws.com")
            .to_string();
        let bucket = self.bucket.clone();
        let prefix = self.prefix.clone();
        Box::pin(async move {
            let list_url = format!("{base}/{bucket}?list-type=2&prefix={prefix}/");
            let mut resp = ureq::get(&list_url)
                .call()
                .map_err(|e| PluginError::ProcessIo(format!("s3 list: {e}")))?;

            let body = resp
                .body_mut()
                .read_to_string()
                .map_err(|e| PluginError::ProcessIo(format!("s3 read list: {e}")))?;

            // Parse XML key list (simplified — real impl would use an XML parser).
            let mut records = Vec::new();
            for key in extract_s3_keys(&body) {
                if let Some(sid) = key
                    .strip_prefix(&format!("{prefix}/"))
                    .and_then(|s| s.strip_suffix(".json"))
                {
                    let obj_url = format!("{base}/{bucket}/{key}");
                    if let Ok(mut obj_resp) = ureq::get(&obj_url).call()
                        && let Ok(obj_str) = obj_resp.body_mut().read_to_string()
                        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&obj_str)
                    {
                        records.push(StorageRecord {
                            session_id: sid.to_string(),
                            project: val["project"].as_str().map(String::from),
                            data: val["data"].clone(),
                        });
                    }
                }
            }

            Ok(records)
        })
    }

    fn delete(
        &mut self,
        session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, PluginError>> + Send + '_>> {
        let url = self.object_url(session_id);
        Box::pin(async move {
            match ureq::delete(&url).call() {
                Ok(_) => Ok(true),
                Err(ureq::Error::StatusCode(404)) => Ok(false),
                Err(e) => Err(PluginError::ProcessIo(format!("s3 delete: {e}"))),
            }
        })
    }

    fn name(&self) -> &str {
        "s3"
    }
}

/// Extract S3 object keys from a ListObjectsV2 XML response.
fn extract_s3_keys(xml: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<Key>") {
        let key_start = pos + start + 5;
        if let Some(end) = xml[key_start..].find("</Key>") {
            keys.push(xml[key_start..key_start + end].to_string());
            pos = key_start + end + 6;
        } else {
            break;
        }
    }
    keys
}

#[cfg(test)]
#[path = "storage_trait.tests.rs"]
mod tests;
