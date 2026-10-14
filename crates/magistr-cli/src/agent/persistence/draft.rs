//! Draft payloads are children of the conversation history record. All writes
//! use one transaction and a revision comparison; a delayed writer cannot restore
//! an older draft or replace an edit made by another application.
use base64::{Engine, engine::general_purpose::STANDARD};
use magistr_application::sessions::{
    MAX_ATTACHMENT_TOTAL_BYTES, MAX_PROMPT_BYTES, PromptAttachment, PromptInput,
};
use magistr_workspace::file_content::FileContent;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{session_record::SessionRecord, storage};
use crate::{
    application::{DraftStatus, SavedDraft, StagedAttachment},
    config::inputs::ProjectInputs,
};

pub(crate) fn read_failure(error: impl std::fmt::Display) -> String {
    tracing::warn!(%error, "could not read saved draft");
    "Your saved message could not be read. Try again.".into()
}

pub(crate) fn save_failure(error: impl std::fmt::Display) -> String {
    tracing::warn!(%error, "could not save draft");
    "Your message is still open, but it could not be saved. Try again.".into()
}

const MAX_ENCODED_BYTES: usize = 2 * MAX_ATTACHMENT_TOTAL_BYTES;
pub(crate) const STALE: &str =
    "This draft changed while it was being saved. Read the latest draft before editing again.";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attachment {
    id: String,
    name: String,
    hash: String,
    path: Option<String>,
    content: String,
}

pub(crate) fn load(conn: &Connection, id: &str) -> Result<SavedDraft, String> {
    let lengths: Option<(i64, i64)> = conn.query_row(
        "SELECT length(CAST(text AS BLOB)), length(attachments) FROM session_drafts WHERE session_id = ?1",
        [id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(read_failure)?;
    let Some((text_len, attachment_len)) = lengths else {
        return Ok(SavedDraft {
            status: DraftStatus {
                conversation: id.to_owned(),
                ..DraftStatus::default()
            },
            ..SavedDraft::default()
        });
    };
    if text_len < 0
        || text_len > MAX_PROMPT_BYTES as i64
        || attachment_len < 0
        || attachment_len > MAX_ENCODED_BYTES as i64
    {
        return Err(
            "The saved message exceeds the supported size. Its original contents have been kept."
                .into(),
        );
    }
    let (revision, text, bytes): (i64, String, Vec<u8>) = conn
        .query_row(
            "SELECT revision, text, attachments FROM session_drafts WHERE session_id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(read_failure)?;
    let revision = u64::try_from(revision).map_err(read_failure)?;
    let stored: Vec<Attachment> = serde_json::from_slice(&bytes).map_err(read_failure)?;
    if stored.len() > magistr_application::sessions::MAX_ATTACHMENTS {
        return Err("The saved message contains too many files.".into());
    }
    let attachments = stored
        .into_iter()
        .map(|file| {
            let bytes = STANDARD
                .decode(file.content)
                .map_err(|_| "A saved file could not be read.".to_owned())?;
            if FileContent::decode(bytes.clone()).content_hash() != file.hash {
                return Err(
                    "A saved file changed in storage. Its original record has been kept.".into(),
                );
            }
            let content =
                PromptAttachment::restore_selection(file.id, file.name, file.hash, bytes)?;
            Ok(StagedAttachment::new(content, file.path))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let draft = SavedDraft {
        text,
        attachments,
        status: DraftStatus {
            conversation: id.to_owned(),
            revision,
            saved_revision: revision,
            save_attempt: 0,
            error: None,
        },
    };
    validate(&draft)?;
    Ok(draft)
}

fn validate(draft: &SavedDraft) -> Result<(), String> {
    PromptInput {
        text: draft.text.clone(),
        attachments: draft
            .attachments
            .iter()
            .map(|file| file.content().clone())
            .collect(),
    }
    .validate(true)
}

pub(crate) fn write(
    conn: &Connection,
    record: &SessionRecord,
    draft: &SavedDraft,
    expected: u64,
) -> Result<(), String> {
    validate(draft)?;
    let revision = i64::try_from(draft.status.revision)
        .map_err(|_| "The saved message has too many edits.")?;
    let expected =
        i64::try_from(expected).map_err(|_| "The saved message has an invalid revision.")?;
    let attachments = draft
        .attachments
        .iter()
        .map(|file| Attachment {
            id: file.id().to_owned(),
            name: file.content().name().to_owned(),
            hash: file.content().content_hash().to_owned(),
            path: file.path().map(str::to_owned),
            content: STANDARD.encode(file.content().bytes()),
        })
        .collect::<Vec<_>>();
    let payload = serde_json::to_vec(&attachments).map_err(save_failure)?;
    if payload.len() > MAX_ENCODED_BYTES {
        return Err("The saved files exceed the supported size.".into());
    }
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(save_failure)?;
    let result = (|| {
        let current: Option<i64> = conn
            .query_row(
                "SELECT revision FROM session_drafts WHERE session_id = ?1",
                [&record.session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(save_failure)?;
        if current.unwrap_or(0) != expected {
            return Err(STALE.into());
        }
        storage::insert_session(conn, record).map_err(save_failure)?;
        conn.execute(
            "INSERT INTO session_drafts(session_id, revision, text, attachments) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id) DO UPDATE SET revision=excluded.revision, text=excluded.text, attachments=excluded.attachments",
            params![record.session_id, revision, draft.text, payload],
        ).map_err(save_failure)?;
        Ok(())
    })();
    match result {
        Ok(()) => conn.execute_batch("COMMIT;").map_err(save_failure),
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(error)
        }
    }
}

pub(crate) enum SavedEdit {
    Text(String),
    Discard,
    Remove(String),
}

pub(crate) fn edit_saved(
    inputs: &ProjectInputs,
    id: &str,
    expected: u64,
    edit: SavedEdit,
) -> Result<DraftStatus, String> {
    let path = storage::sessions_db_path(inputs)
        .ok_or_else(|| "No folder is available for saved conversations.".to_owned())?;
    // Editing saved history cannot create an unrelated empty database.
    let reader = storage::open_readonly_db(&path).map_err(read_failure)?;
    let mut record = storage::get_session(&reader, id)
        .map_err(read_failure)?
        .ok_or_else(|| "This saved conversation is no longer available.".to_owned())?;
    let mut draft = load(&reader, id)?;
    if draft.status.revision != expected {
        return Err(STALE.into());
    }
    match edit {
        SavedEdit::Text(text) => draft.text = text,
        SavedEdit::Discard => {
            draft.text.clear();
            draft.attachments.clear();
        }
        SavedEdit::Remove(id) => {
            let at = draft
                .attachments
                .iter()
                .position(|file| file.id() == id)
                .ok_or_else(|| {
                    "This file is no longer attached to the saved message.".to_owned()
                })?;
            draft.attachments.remove(at);
        }
    }
    draft.status.revision = draft
        .status
        .revision
        .checked_add(1)
        .ok_or_else(|| "The saved message has too many edits.".to_owned())?;
    validate(&draft)?;
    record.updated_at = chrono::Utc::now();
    drop(reader);
    let conn = storage::open_db(&path).map_err(save_failure)?;
    write(&conn, &record, &draft, expected)?;
    Ok(DraftStatus {
        saved_revision: draft.status.revision,
        ..draft.status
    })
}

#[cfg(test)]
#[path = "draft.tests.rs"]
mod tests;
