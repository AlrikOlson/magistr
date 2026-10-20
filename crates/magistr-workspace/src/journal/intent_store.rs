use super::{JournalError, JournalWriteOutcome};
use rusqlite::{Connection, OptionalExtension, params};

/// Native v3 additions; original v1/v2 rows and compressed bytes are unchanged.
pub(super) struct IntentStore;

impl IntentStore {
    pub(super) const SCHEMA: [(&'static str, &'static str); 2] = [
        (
            "native_write_intents",
            "CREATE TABLE native_write_intents(op_seq INTEGER PRIMARY KEY REFERENCES ops(seq),outcome TEXT NOT NULL CHECK(outcome IN ('pending','applied','rolled_back','indeterminate')))",
        ),
        (
            "native_intent_undo",
            "CREATE TABLE native_intent_undo(intent_seq INTEGER NOT NULL REFERENCES ops(seq),op_seq INTEGER NOT NULL REFERENCES ops(seq),PRIMARY KEY(intent_seq,op_seq))",
        ),
    ];

    pub(super) fn create(db: &Connection) -> Result<(), JournalError> {
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 2 {
            for (_, sql) in Self::SCHEMA {
                db.execute_batch(sql)?;
            }
            db.execute_batch("PRAGMA user_version=3")?;
        } else if !matches!(version, 3 | 4) {
            return Err(JournalError::UnsupportedVersion(format!(
                "cannot append intent to version {version}"
            )));
        }
        Ok(())
    }

    pub(super) fn record(
        db: &Connection,
        seq: i64,
        undo_of: Option<&[i64]>,
    ) -> Result<(), JournalError> {
        db.execute(
            "INSERT INTO native_write_intents(op_seq,outcome) VALUES (?,'pending')",
            [seq],
        )?;
        for original in undo_of.into_iter().flatten() {
            let eligible:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM ops WHERE seq=? AND is_undo=0 AND seq NOT IN (SELECT op_seq FROM undone) AND seq NOT IN (SELECT op_seq FROM native_write_intents WHERE outcome!='applied'))",[original],|row|row.get(0))?;
            if !eligible || *original >= seq {
                return Err(JournalError::InvalidInput(format!(
                    "undo operation {original} is not eligible"
                )));
            }
            db.execute(
                "INSERT OR IGNORE INTO native_intent_undo(intent_seq,op_seq) VALUES (?,?)",
                params![seq, original],
            )?;
        }
        Ok(())
    }

    pub(super) fn read(
        db: &Connection,
        seq: i64,
    ) -> Result<Option<JournalWriteOutcome>, JournalError> {
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 3 {
            return Ok(None);
        }
        let value: Option<String> = db
            .query_row(
                "SELECT outcome FROM native_write_intents WHERE op_seq=?",
                [seq],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|value| match value.as_str() {
                "pending" => Ok(JournalWriteOutcome::Pending),
                "applied" => Ok(JournalWriteOutcome::Applied),
                "rolled_back" => Ok(JournalWriteOutcome::RolledBack),
                "indeterminate" => Ok(JournalWriteOutcome::Indeterminate),
                _ => Err(JournalError::CorruptRecord(
                    "unknown native write outcome".into(),
                )),
            })
            .transpose()
    }

    pub(super) fn resolve(
        db: &Connection,
        seq: i64,
        outcome: JournalWriteOutcome,
    ) -> Result<(), JournalError> {
        if outcome == JournalWriteOutcome::Pending {
            return Err(JournalError::InvalidInput(
                "cannot resolve back to pending".into(),
            ));
        }
        let previous = Self::read(db, seq)?;
        if previous == Some(outcome) {
            return Ok(());
        }
        if previous != Some(JournalWriteOutcome::Pending) {
            return Err(JournalError::InvalidInput(
                "only a pending native intent can be resolved".into(),
            ));
        }
        let name = match outcome {
            JournalWriteOutcome::Applied => "applied",
            JournalWriteOutcome::RolledBack => "rolled_back",
            JournalWriteOutcome::Indeterminate => "indeterminate",
            JournalWriteOutcome::Pending => unreachable!("pending refused above"),
        };
        if outcome == JournalWriteOutcome::Applied {
            // Plain INSERT makes a conflicting second undo fail the transaction.
            db.execute("INSERT INTO undone(op_seq,by_seq) SELECT op_seq,intent_seq FROM native_intent_undo WHERE intent_seq=?",[seq])?;
        }
        db.execute(
            "UPDATE native_write_intents SET outcome=? WHERE op_seq=?",
            params![name, seq],
        )?;
        Ok(())
    }
}
