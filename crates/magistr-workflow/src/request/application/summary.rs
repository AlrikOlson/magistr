use serde::Serialize;

use crate::request::domain::{Request, RequestKind, RequestStatus};

/// A bounded request row in an inbox snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RequestSummary {
    pub id: String,
    pub kind: RequestKind,
    pub status: RequestStatus,
    pub from: String,
    pub body: String,
    pub created: String,
}

impl From<&Request> for RequestSummary {
    fn from(request: &Request) -> Self {
        const BODY_LEN: usize = 140;
        let body = if request.body.chars().count() <= BODY_LEN {
            request.body.clone()
        } else {
            let cut: String = request.body.chars().take(BODY_LEN).collect();
            format!("{}…", cut.trim_end())
        };
        Self {
            id: request.id.clone(),
            kind: request.kind,
            status: request.status,
            from: request.from.clone(),
            body,
            created: request.created.clone(),
        }
    }
}
