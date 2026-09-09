//! Enterprise directory integration — SAML, LDAP and SCIM over one
//! group→role mapping.
//!
//! [`auth`](super::auth) answers *who somebody is*; this module answers
//! *what their organization says they may do*. The three protocols an
//! enterprise brings are three transports into a single question —
//! which groups is this person in — so the mapping ([`GroupRole`])
//! lives once in [`DirectoryConfig`] and each protocol is a way of
//! answering it:
//!
//! - **SAML** ([`SamlAssertion`]) — the IdP asserts the groups, in an
//!   assertion whose conditions, audience and issuer are checked
//!   against [`SamlConfig`].
//! - **LDAP** ([`LdapConfig`], [`GroupSource`]) — magistr asks, and the
//!   directory answers with `memberOf` DNs.
//! - **SCIM** ([`ScimRequest`], [`apply_scim`]) — the IdP pushes, and
//!   the roster in `.magistr/team.json` is written rather than
//!   hand-edited.
//!
//! Where several groups map to different roles the **most privileged**
//! wins ([`GroupRole::highest`]), which is the same reason
//! [`PolicyEngine`] folds a maximum over severity: an extremum makes
//! the order the rows were written in unable to change the answer.
//!
//! # Failing closed
//!
//! The rule the whole module is shaped around is one distinction: **an
//! answered "no" is data, an unanswered question is a refusal.**
//! [`DirectoryOutcome`] names the three cases, [`GroupSource`]'s return
//! type is what forces an implementation to tell them apart, and
//! [`resolve_role_with_directory`] is where the refusal happens — before
//! the CI shortcut, because automation slipping past a directory outage
//! is precisely the implicit grant being refused.
//!
//! # What does not ship here
//!
//! No XML signature verification: an assertion's conditions, audience
//! and issuer are checked, and the signature is not. That is the identity resolver's
//! `id_token` shortcut again and it is filed the same way. The
//! structural half of the defence *is* here — a document carrying more
//! than one `<Assertion>`, or any DOCTYPE, is refused outright, which
//! is the XML Signature Wrapping attack's own precondition.
//!
//! No live LDAP transport: [`GroupSource`] is the port, and no
//! implementation of it speaks LDAP yet. What ships is the part most
//! integrations get wrong anyway — filter escaping and DN handling.
//!
//! [`PolicyEngine`]: magistr_engine::policy::PolicyEngine

mod apply_scim;
mod directory_config;
mod directory_outcome;
mod group_role;
mod group_source;
mod ldap_config;
mod resolve_role_with_directory;
mod saml_assertion;
mod saml_config;
mod scim_effect;
mod scim_name;
mod scim_operation;
mod scim_outcome;
mod scim_request;
mod scim_user;
mod scim_value;

pub use apply_scim::apply_scim;
pub use directory_config::DirectoryConfig;
pub use directory_outcome::DirectoryOutcome;
pub use group_role::GroupRole;
pub use group_source::GroupSource;
pub use ldap_config::LdapConfig;
pub use resolve_role_with_directory::resolve_role_with_directory;
pub use saml_assertion::SamlAssertion;
pub use saml_config::SamlConfig;
pub use scim_effect::ScimEffect;
pub use scim_name::ScimName;
pub use scim_operation::ScimOperation;
pub use scim_outcome::ScimOutcome;
pub use scim_request::ScimRequest;
pub use scim_user::ScimUser;
pub use scim_value::ScimValue;
