//! A SCIM provisioning request, as one document.

use serde::{Deserialize, Serialize};

use super::scim_operation::ScimOperation;
use super::scim_user::ScimUser;

/// One SCIM request against the roster.
///
/// magistr runs no HTTP listener, so a request arrives as a document
/// rather than as a live call: the `op` tag carries what the verb and
/// the path would have said, and the rest is the body the IdP sent.
/// That keeps the applying logic — [`apply_scim`] — a pure function of
/// a request and a roster, which is what makes it testable without a
/// server, and leaves the transport (a webhook relay, a CI job, a
/// server route) a separate and replaceable thing.
///
/// [`apply_scim`]: super::apply_scim
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum ScimRequest {
    /// `POST /Users` — provision somebody new.
    Create {
        /// The user resource to create.
        user: ScimUser,
    },
    /// `PUT /Users/{id}` — replace an existing user wholesale.
    Replace {
        /// The member id being replaced.
        id: String,
        /// The full replacement resource.
        user: ScimUser,
    },
    /// `PATCH /Users/{id}` — the deactivation path both Okta and Entra
    /// actually use.
    Patch {
        /// The member id being patched.
        id: String,
        /// The operations the provider sent.
        #[serde(rename = "Operations", alias = "operations")]
        operations: Vec<ScimOperation>,
    },
    /// `DELETE /Users/{id}` — a hard delete, which is the one thing
    /// providers rarely send.
    Delete {
        /// The member id being deleted.
        id: String,
    },
}

impl ScimRequest {
    /// The member id this request is about.
    #[must_use]
    pub fn member_id(&self) -> &str {
        match self {
            ScimRequest::Create { user } => user.member_id(),
            ScimRequest::Replace { id, .. }
            | ScimRequest::Patch { id, .. }
            | ScimRequest::Delete { id } => id.trim(),
        }
    }
}
