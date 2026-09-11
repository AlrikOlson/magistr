use std::io::{Cursor, Read};

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};

use crate::file_content::FileContent;

use super::{
    JournalError, JournalFileKind, JournalFileRecord, JournalLogEntry, JournalLogQuery, JournalOp,
    UndoCandidateQuery,
};

const MAX_ROWS: usize = 1000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_STRING: usize = 1024 * 1024;

/// One bounded hydration context, used identically by writer and read-only views.
pub(super) struct JournalQueries<'a> {
    db: &'a Connection,
    remaining: usize,
    remaining_files: usize,
}

impl<'a> JournalQueries<'a> {
    pub(super) fn new(db: &'a Connection) -> Self {
        Self {
            db,
            remaining: MAX_BYTES,
            remaining_files: MAX_ROWS,
        }
    }

    pub(super) fn get(&mut self, seq: i64) -> Result<Option<JournalOp>, JournalError> {
        let mut statement = self
            .db
            .prepare("SELECT seq,at,tool,args_hash,session_id,is_undo FROM ops WHERE seq=?")?;
        let mut rows = statement.query([seq])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let seq = row.get(0)?;
        let at: i64 = row.get(1)?;
        let tool = self.string(row, 2)?;
        let args_hash = self.string(row, 3)?;
        let session_id = self.string(row, 4)?;
        let is_undo: i64 = row.get(5)?;
        if seq < 1 || at < 0 || !matches!(is_undo, 0 | 1) {
            return Err(JournalError::CorruptRecord(
                "invalid operation sequence, timestamp, or undo flag".to_owned(),
            ));
        }
        let undo_of = if is_undo == 1 {
            let sql = if super::intent_store::IntentStore::read(self.db, seq)?.is_some() {
                "SELECT op_seq FROM native_intent_undo WHERE intent_seq=? ORDER BY op_seq LIMIT 1001"
            } else {
                "SELECT op_seq FROM undone WHERE by_seq=? ORDER BY op_seq LIMIT 1001"
            };
            Some(self.seqs(sql, vec![Value::Integer(seq)])?)
        } else {
            None
        };
        let mut files = Vec::new();
        let mut statement = self.db.prepare("SELECT ord,kind,path,before_hash,after_hash FROM op_files WHERE op_seq=? ORDER BY ord LIMIT 1001")?;
        let mut rows = statement.query([seq])?;
        while let Some(row) = rows.next()? {
            if self.remaining_files == 0 {
                return Err(JournalError::ReadLimitExceeded(
                    "more than 1000 file records; request a smaller page".to_owned(),
                ));
            }
            self.remaining_files -= 1;
            let ord: i64 = row.get(0)?;
            if ord != files.len() as i64 {
                return Err(JournalError::CorruptRecord(
                    "file ordinals are not contiguous".to_owned(),
                ));
            }
            let kind = match self.string(row, 1)?.as_str() {
                "create" => JournalFileKind::Create,
                "write" => JournalFileKind::Write,
                "delete" => JournalFileKind::Delete,
                _ => return Err(JournalError::CorruptRecord("unknown file kind".to_owned())),
            };
            let path = self.string(row, 2)?;
            let before_hash = self.string(row, 3)?;
            let after_hash = self.string(row, 4)?;
            let before = self.blob(&before_hash)?;
            let after = self.blob(&after_hash)?;
            files.push(JournalFileRecord {
                kind,
                path,
                before_hash,
                after_hash,
                before,
                after,
            });
        }
        Ok(Some(JournalOp {
            seq,
            at,
            tool,
            args_hash,
            session_id,
            undo_of,
            files,
        }))
    }

    pub(super) fn list(&mut self, since_seq: i64) -> Result<Vec<JournalOp>, JournalError> {
        let seqs = self.seqs(
            "SELECT seq FROM ops WHERE seq>=? ORDER BY seq LIMIT 1001",
            vec![Value::Integer(since_seq)],
        )?;
        self.hydrate(seqs)
    }

    pub(super) fn recent(
        &mut self,
        query: &JournalLogQuery,
    ) -> Result<Vec<JournalLogEntry>, JournalError> {
        validate_limit(query.limit)?;
        validate_session(query.session_id.as_deref())?;
        let mut clauses = Vec::new();
        let mut values = Vec::new();
        if let Some(since) = query.since_seq.filter(|value| *value > 0) {
            clauses.push("seq>=?");
            values.push(Value::Integer(since));
        }
        if let Some(before) = query.before_seq {
            clauses.push("seq<?");
            values.push(Value::Integer(before));
        }
        if let Some(session) = &query.session_id {
            clauses.push("session_id=?");
            values.push(Value::Text(session.clone()));
        }
        values.push(Value::Integer(query.limit as i64));
        let clause = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let seqs = self.seqs(
            &format!("SELECT seq FROM ops{clause} ORDER BY seq DESC LIMIT ?"),
            values,
        )?;
        let ops = self.hydrate(seqs)?;
        ops.into_iter()
            .map(|op| {
                let undone_by = self
                    .db
                    .query_row(
                        "SELECT by_seq FROM undone WHERE op_seq=?",
                        [op.seq],
                        |row| row.get(0),
                    )
                    .optional()?;
                let write_outcome = super::intent_store::IntentStore::read(self.db, op.seq)?;
                Ok(JournalLogEntry {
                    undoable: op.undo_of.is_none()
                        && undone_by.is_none()
                        && write_outcome
                            .is_none_or(|status| status == super::JournalWriteOutcome::Applied),
                    write_outcome,
                    op,
                    undone_by,
                })
            })
            .collect()
    }

    pub(super) fn candidates(
        &mut self,
        query: &UndoCandidateQuery,
    ) -> Result<Vec<JournalOp>, JournalError> {
        validate_limit(query.limit)?;
        validate_session(query.session_id.as_deref())?;
        let mut clauses = vec![
            "is_undo=0".to_owned(),
            "seq NOT IN (SELECT op_seq FROM undone)".to_owned(),
        ];
        let version: i64 = self
            .db
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 3 {
            clauses.push(
                "seq NOT IN (SELECT op_seq FROM native_write_intents WHERE outcome!='applied')"
                    .to_owned(),
            );
        }
        let mut values = Vec::new();
        if let Some(session) = &query.session_id {
            clauses.push("session_id=?".to_owned());
            values.push(Value::Text(session.clone()));
        }
        if let Some(seqs) = &query.seqs {
            validate_limit(seqs.len())?;
            if seqs.is_empty() {
                return Ok(Vec::new());
            }
            clauses.push(format!("seq IN ({})", vec!["?"; seqs.len()].join(",")));
            values.extend(seqs.iter().copied().map(Value::Integer));
        }
        values.push(Value::Integer(query.limit as i64));
        let seqs = self.seqs(
            &format!(
                "SELECT seq FROM ops WHERE {} ORDER BY seq DESC LIMIT ?",
                clauses.join(" AND ")
            ),
            values,
        )?;
        self.hydrate(seqs)
    }

    fn seqs(&self, sql: &str, values: Vec<Value>) -> Result<Vec<i64>, JournalError> {
        let mut statement = self.db.prepare(sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| row.get(0))?;
        let mut seqs = Vec::new();
        for row in rows {
            if seqs.len() == MAX_ROWS {
                return Err(JournalError::ReadLimitExceeded(
                    "more than 1000 operations; use recent pagination".to_owned(),
                ));
            }
            seqs.push(row?);
        }
        Ok(seqs)
    }

    fn hydrate(&mut self, seqs: Vec<i64>) -> Result<Vec<JournalOp>, JournalError> {
        seqs.into_iter()
            .map(|seq| {
                self.get(seq)?.ok_or_else(|| {
                    JournalError::CorruptRecord(format!(
                        "operation {seq} disappeared from snapshot"
                    ))
                })
            })
            .collect()
    }

    fn string(&mut self, row: &Row<'_>, index: usize) -> Result<String, JournalError> {
        let rusqlite::types::ValueRef::Text(bytes) = row.get_ref(index)? else {
            return Err(JournalError::CorruptRecord(
                "expected text column".to_owned(),
            ));
        };
        if bytes.len() > MAX_STRING || bytes.len() > self.remaining {
            return Err(JournalError::ReadLimitExceeded(
                "metadata exceeds bounded text budget".to_owned(),
            ));
        }
        self.remaining -= bytes.len();
        String::from_utf8(bytes.to_vec())
            .map_err(|_| JournalError::CorruptRecord("metadata is not UTF-8".to_owned()))
    }

    fn blob(&mut self, hash: &str) -> Result<Option<FileContent>, JournalError> {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(JournalError::CorruptBlob {
                hash: hash.to_owned(),
                reason: "invalid SHA-256 address".to_owned(),
            });
        }
        let size: Option<(i64, i64)> = self
            .db
            .query_row(
                "SELECT length(zstd),bytes FROM blobs WHERE hash=?",
                [hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((length, declared)) = size else {
            return Ok(None);
        };
        if length < 0 || length != declared {
            return Err(JournalError::CorruptBlob {
                hash: hash.to_owned(),
                reason: "stored compressed byte count is inconsistent".to_owned(),
            });
        }
        if length as u64 > MAX_BYTES as u64 {
            return Err(JournalError::ReadLimitExceeded(
                "compressed blob exceeds 64 MiB".to_owned(),
            ));
        }
        let compressed: Vec<u8> = self.db.query_row(
            "SELECT zstd FROM blobs WHERE hash=?",
            params![hash],
            |row| row.get(0),
        )?;
        let failure = |error: std::io::Error| JournalError::CorruptBlob {
            hash: hash.to_owned(),
            reason: format!("invalid zstd content: {error}"),
        };
        let mut decoder =
            zstd::stream::read::Decoder::new(Cursor::new(compressed)).map_err(failure)?;
        decoder.window_log_max(26).map_err(failure)?;
        let mut bytes = Vec::new();
        decoder
            .take((self.remaining + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(failure)?;
        if bytes.len() > self.remaining {
            return Err(JournalError::ReadLimitExceeded(
                "hydrated snapshots exceed 64 MiB; request a smaller page".to_owned(),
            ));
        }
        self.remaining -= bytes.len();
        let content = FileContent::decode(bytes);
        if content.content_hash() != hash {
            return Err(JournalError::CorruptBlob {
                hash: hash.to_owned(),
                reason: "SHA-256 does not match decompressed bytes".to_owned(),
            });
        }
        Ok(Some(content))
    }
}

fn validate_limit(limit: usize) -> Result<(), JournalError> {
    if limit > MAX_ROWS {
        return Err(JournalError::InvalidInput(
            "query limit exceeds 1000".to_owned(),
        ));
    }
    Ok(())
}

fn validate_session(session: Option<&str>) -> Result<(), JournalError> {
    if session.is_some_and(|session| session.len() > MAX_STRING) {
        return Err(JournalError::InvalidInput(
            "session exceeds 1 MiB".to_owned(),
        ));
    }
    Ok(())
}
