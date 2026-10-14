//! Read saved conversations from the session owner's store without starting a writer.
use crate::{
    config::inputs::ProjectInputs,
    session::{session_filter::SessionFilter, session_record::SessionRecord, storage},
};

pub fn saved_conversations(inputs: &ProjectInputs) -> Result<Vec<SessionRecord>, String> {
    let path = storage::sessions_db_path(inputs)
        .ok_or_else(|| "No folder is available for saved conversations.".to_owned())?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let conn = storage::open_readonly_db(&path).map_err(|error| error.to_string())?;
    storage::query_sessions(&conn, &SessionFilter::new().with_limit(100))
        .map_err(|error| error.to_string())
}
#[cfg(test)]
#[path = "saved_conversations.tests.rs"]
mod tests;
