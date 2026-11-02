use crate::plan::domain::{Focus, FocusState, WorkstreamCatalog};

use super::WorkstreamInspection;

/// A saved lane focus and the current plan observed under one owner lock.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FocusRead {
    pub lane: String,
    pub focus: Option<Focus>,
    pub focus_state: FocusState,
    pub frontier: WorkstreamInspection,
    pub workstreams: WorkstreamCatalog,
    pub persistent: bool,
}
