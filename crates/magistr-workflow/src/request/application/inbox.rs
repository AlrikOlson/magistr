use std::collections::BTreeMap;

use serde::Serialize;

use super::RequestSummary;
use crate::request::domain::{RequestStatus, Requests};

/// Counts and bounded recent records for the native request inbox.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RequestInbox {
    pub project_id: String,
    pub counts: BTreeMap<&'static str, usize>,
    pub requests: Vec<RequestSummary>,
    pub omitted: usize,
}

impl From<&Requests> for RequestInbox {
    fn from(store: &Requests) -> Self {
        const LIST_CAP: usize = 60;
        let counts = [
            ("new", RequestStatus::New),
            ("triaged", RequestStatus::Triaged),
            ("researched", RequestStatus::Researched),
            ("raised", RequestStatus::Raised),
            ("promoted", RequestStatus::Promoted),
            ("dismissed", RequestStatus::Dismissed),
        ]
        .into_iter()
        .map(|(name, status)| {
            (
                name,
                store.requests.iter().filter(|s| s.status == status).count(),
            )
        })
        .chain([("total", store.requests.len())])
        .collect();
        let mut recent: Vec<_> = store.requests.iter().collect();
        recent.sort_by(|a, b| b.created.cmp(&a.created));
        Self {
            project_id: store.project_id.clone(),
            counts,
            requests: recent
                .into_iter()
                .take(LIST_CAP)
                .map(RequestSummary::from)
                .collect(),
            omitted: store.requests.len().saturating_sub(LIST_CAP),
        }
    }
}
