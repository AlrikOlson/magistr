//! What a team must know to trust a SAML assertion.

use serde::{Deserialize, Serialize};

/// The SAML half of a team's `directory` block.
///
/// Every field here is a thing an assertion is checked *against*: our
/// own entity id (the audience it must name), the IdP's entity id (the
/// issuer it must carry), the skew we allow its validity window, and
/// which attribute carries group membership. Nothing here is optional
/// decoration — an assertion that is not checked against a configured
/// audience is an assertion any IdP-signed party can replay at us.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamlConfig {
    /// Our own SP entity id. The assertion's `<Audience>` must name it.
    pub entity_id: String,
    /// The IdP's entity id. The assertion's `<Issuer>` must be it.
    pub idp_entity_id: String,
    /// Clock skew allowed on the validity window, in seconds.
    ///
    /// Two minutes by default: the common IdP window is five, and the
    /// skew is a tolerance on top of it rather than an extension of it.
    #[serde(default = "default_skew_secs")]
    pub clock_skew_secs: u32,
    /// The assertion attribute carrying group membership.
    #[serde(default = "default_groups_attribute")]
    pub groups_attribute: String,
    /// The attribute carrying the email; the `NameID` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_attribute: Option<String>,
    /// The attribute carrying a display name, when the IdP sends one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_attribute: Option<String>,
}

fn default_skew_secs() -> u32 {
    120
}

fn default_groups_attribute() -> String {
    "groups".to_string()
}

impl SamlConfig {
    /// A config for an SP/IdP pair, with the defaults for everything
    /// else.
    #[must_use]
    pub fn new(entity_id: impl Into<String>, idp_entity_id: impl Into<String>) -> Self {
        SamlConfig {
            entity_id: entity_id.into(),
            idp_entity_id: idp_entity_id.into(),
            clock_skew_secs: default_skew_secs(),
            groups_attribute: default_groups_attribute(),
            email_attribute: None,
            name_attribute: None,
        }
    }

    /// The provider key a SAML-sourced identity carries, so it is told
    /// apart from a self-asserted one and from a device-flow login.
    #[must_use]
    pub fn provider_key(&self) -> String {
        format!("saml:{}", self.idp_entity_id.trim_end_matches('/'))
    }
}
