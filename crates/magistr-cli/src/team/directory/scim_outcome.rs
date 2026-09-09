//! The report a SCIM request leaves behind.

use crate::team::TeamRole;

use super::scim_effect::ScimEffect;

/// What applying a SCIM request did, and what it did not do.
///
/// `ignored_paths` is the honest half. A roster row is an id, a name, a
/// role and whether the person is provisioned, so a PATCH of `title` or
/// `locale` genuinely changes nothing here — but "changed nothing"
/// reported as success is how a provisioning integration drifts without
/// anyone noticing, so the paths that were not acted on are named
/// rather than dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScimOutcome {
    /// The roster member id the request was about.
    pub id: String,
    /// What happened to the roster.
    pub effect: ScimEffect,
    /// The role the member holds after the request.
    pub role: TeamRole,
    /// Attribute paths the request named that the roster does not
    /// model, in the order they were sent.
    pub ignored_paths: Vec<String>,
}

impl ScimOutcome {
    /// A report with nothing ignored.
    #[must_use]
    pub fn new(id: impl Into<String>, effect: ScimEffect, role: TeamRole) -> Self {
        ScimOutcome {
            id: id.into(),
            effect,
            role,
            ignored_paths: Vec::new(),
        }
    }

    /// The same report, naming the paths that were not acted on.
    #[must_use]
    pub fn ignoring(mut self, paths: Vec<String>) -> Self {
        self.ignored_paths = paths;
        self
    }
}
