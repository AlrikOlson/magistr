//! A roadmap request and the real conversation range that answered it.

use super::request_phase::RequestPhase;

/// One user request, retained on Roadmap across work and navigation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadmapRequest {
    /// The user's message, unchanged.
    pub text: String,
    /// First response source line; absent until a session accepts the request.
    pub source_start: Option<usize>,
    /// Exclusive end of the response; absent while it is receiving events.
    pub source_end: Option<usize>,
    /// Delivery, execution, or terminal outcome.
    pub phase: RequestPhase,
    /// An actual reported failure, if present.
    pub error: Option<String>,
}

#[cfg(test)]
#[path = "roadmap_request.tests.rs"]
mod tests;

impl RoadmapRequest {
    pub(super) fn new(text: String) -> Self {
        Self {
            text,
            source_start: None,
            source_end: None,
            phase: RequestPhase::Waiting,
            error: None,
        }
    }
}
