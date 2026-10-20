use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    JournalError, intent_store::IntentStore, journal_lifecycle::OpenedJournal,
    journal_queries::JournalQueries,
};

/// Imports a retired workspace once into the admitted writer. The marker and
/// every row commit together; a failed or interrupted import can be retried.
pub(super) struct JournalImport;

impl JournalImport {
    pub(super) const SCHEMA: [(&'static str, &'static str); 2] = [
        (
            "imported_ops",
            "CREATE TABLE imported_ops(op_seq INTEGER PRIMARY KEY REFERENCES ops(seq),position INTEGER NOT NULL UNIQUE CHECK(position<0))",
        ),
        (
            "journal_upgrades",
            "CREATE TABLE journal_upgrades(name TEXT PRIMARY KEY)",
        ),
    ];

    pub(super) fn completed(db: &Connection) -> Result<bool, JournalError> {
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(version >= 4
            && db.query_row(
                "SELECT EXISTS(SELECT 1 FROM journal_upgrades WHERE name='workbench')",
                [],
                |row| row.get(0),
            )?)
    }

    pub(super) fn order(db: &Connection) -> Result<&'static str, JournalError> {
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(if version >= 4 {
            "COALESCE((SELECT position FROM imported_ops WHERE op_seq=ops.seq),seq)"
        } else {
            "seq"
        })
    }

    pub(super) fn position(db: &Connection, seq: i64) -> Result<i64, JournalError> {
        if Self::order(db)? == "seq" {
            return Ok(seq);
        }
        Ok(db
            .query_row(
                "SELECT position FROM imported_ops WHERE op_seq=?",
                [seq],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(seq))
    }

    pub(super) fn run(db: &mut Connection, path: &Path) -> Result<(), JournalError> {
        if Self::completed(db)? {
            return Ok(());
        }
        // SQLite reads a consistent snapshot, including committed WAL contents.
        // The caller holds both workspace admissions; raw database files are never copied.
        let source = OpenedJournal::reader(path)?;
        let source = source.unchecked_transaction()?;
        let version: i64 = source.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 3 {
            return Err(JournalError::UnsupportedVersion(
                "unrecognized older workspace journal; original retained".into(),
            ));
        }
        let foreign_error = source.prepare("PRAGMA foreign_key_check")?.exists([])?;
        let broken_undo: bool = source.query_row("SELECT EXISTS(SELECT 1 FROM undone WHERE op_seq NOT IN (SELECT seq FROM ops) OR by_seq NOT IN (SELECT seq FROM ops))", [], |row| row.get(0))?;
        if foreign_error || broken_undo {
            return Err(JournalError::CorruptRecord(
                "older journal contains broken operation references; original retained".into(),
            ));
        }
        let last: i64 =
            source.query_row("SELECT COALESCE(MAX(seq),0) FROM ops", [], |row| row.get(0))?;
        let tx = db.transaction()?;
        let offset: i64 =
            tx.query_row("SELECT COALESCE(MAX(seq),0) FROM ops", [], |row| row.get(0))?;
        offset.checked_add(last).ok_or_else(|| {
            JournalError::InvalidInput("combined journal exceeds operation ID range".into())
        })?;
        IntentStore::create(&tx)?;
        for (_, sql) in Self::SCHEMA {
            tx.execute_batch(sql)?;
        }
        tx.execute_batch("PRAGMA user_version=4")?;
        let mut statement = source.prepare("SELECT seq FROM ops ORDER BY seq")?;
        let seqs = statement.query_map([], |row| row.get::<_, i64>(0))?;
        for seq in seqs {
            let seq = seq?;
            let op = JournalQueries::new(&source)
                .get(seq)?
                .ok_or_else(|| JournalError::CorruptRecord("older operation disappeared".into()))?;
            let id = offset + seq;
            tx.execute(
                "INSERT INTO ops(seq,at,tool,args_hash,session_id,is_undo) VALUES (?,?,?,?,?,?)",
                params![
                    id,
                    op.at,
                    op.tool,
                    op.args_hash,
                    op.session_id,
                    op.undo_of.is_some()
                ],
            )?;
            tx.execute(
                "INSERT INTO imported_ops(op_seq,position) VALUES (?,?)",
                params![id, seq - last - 1],
            )?;
            for (ord, file) in op.files.iter().enumerate() {
                tx.execute("INSERT INTO op_files(op_seq,ord,kind,path,before_hash,after_hash) VALUES (?,?,?,?,?,?)",
                    params![id, ord as i64, file.kind.as_str(), file.path, file.before_hash, file.after_hash])?;
            }
        }
        Self::blobs(&source, &tx, last)?;
        Self::links(&source, &tx, offset, version)?;
        tx.execute(
            "INSERT INTO journal_upgrades(name) VALUES ('workbench')",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn blobs(source: &Connection, target: &Connection, last: i64) -> Result<(), JournalError> {
        let mut statement = source.prepare("SELECT hash,zstd,bytes,last_ref_seq FROM blobs")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let hash: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let declared: i64 = row.get(2)?;
            let reference: i64 = row.get(3)?;
            if declared != bytes.len() as i64 || !(1..=last).contains(&reference) {
                return Err(JournalError::CorruptRecord(
                    "older journal has invalid blob accounting; original retained".into(),
                ));
            }
            // Keep existing blobs and their more recent access order. Imported
            // snapshots become older than every current operation, regardless of ID.
            target.execute("INSERT INTO blobs(hash,zstd,bytes,last_ref_seq) VALUES (?,?,?,?) ON CONFLICT(hash) DO UPDATE SET last_ref_seq=MAX(blobs.last_ref_seq,excluded.last_ref_seq)",
                params![hash, bytes, declared, reference - last - 1])?;
        }
        Ok(())
    }

    fn links(
        source: &Connection,
        target: &Connection,
        offset: i64,
        version: i64,
    ) -> Result<(), JournalError> {
        for (table, a, b) in [
            ("undone", "op_seq", "by_seq"),
            ("native_intent_undo", "intent_seq", "op_seq"),
        ] {
            if version < 3 && table == "native_intent_undo" {
                continue;
            }
            let mut statement = source.prepare(&format!("SELECT {a},{b} FROM {table}"))?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                target.execute(
                    &format!("INSERT INTO {table}({a},{b}) VALUES (?,?)"),
                    params![
                        row.get::<_, i64>(0)? + offset,
                        row.get::<_, i64>(1)? + offset
                    ],
                )?;
            }
        }
        if version >= 3 {
            let mut statement =
                source.prepare("SELECT op_seq,outcome FROM native_write_intents")?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                target.execute(
                    "INSERT INTO native_write_intents(op_seq,outcome) VALUES (?,?)",
                    params![row.get::<_, i64>(0)? + offset, row.get::<_, String>(1)?],
                )?;
            }
        }
        Ok(())
    }
}
