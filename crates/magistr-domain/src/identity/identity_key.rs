use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::IdentityError;

/// Validated, opaque local key; parsing never normalizes the supplied spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct IdentityKey(String);

impl IdentityKey {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        Self::try_from(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for IdentityKey {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(IdentityError::Empty);
        }
        if value.chars().any(char::is_control) {
            return Err(IdentityError::ControlCharacter);
        }
        Ok(Self(value))
    }
}

impl From<IdentityKey> for String {
    fn from(value: IdentityKey) -> Self {
        value.0
    }
}
