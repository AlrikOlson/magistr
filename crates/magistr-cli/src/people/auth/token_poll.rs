//! One poll of the token endpoint, classified.

use serde_json::Value;

use crate::error::MagistrError;

use super::granted_token::GrantedToken;

/// The outcome of a single token-endpoint poll (RFC 8628 §3.5).
///
/// The two "keep going" cases are distinct because `slow_down` also
/// mutates the schedule: the interval grows by five seconds and stays
/// grown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPoll {
    /// The human has not finished yet — poll again at the same interval.
    Pending,
    /// Poll again, but five seconds slower from here on.
    SlowDown,
    /// The human refused, or the provider revoked the request.
    Denied,
    /// The device code aged out before the human acted.
    Expired,
    /// The provider issued a token.
    Granted(Box<GrantedToken>),
}

impl TokenPoll {
    /// Classify a token-endpoint response body.
    ///
    /// A body carrying an `error` the spec does not define is a hard
    /// failure, not a retry: polling forever on an unknown error is how
    /// a device flow hangs.
    pub fn from_json(value: &Value) -> Result<Self, MagistrError> {
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return match error {
                "authorization_pending" => Ok(TokenPoll::Pending),
                "slow_down" => Ok(TokenPoll::SlowDown),
                "access_denied" => Ok(TokenPoll::Denied),
                "expired_token" => Ok(TokenPoll::Expired),
                other => Err(MagistrError::Other(describe(other, value))),
            };
        }
        let access_token = value
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                MagistrError::Other(
                    "token response carried neither an access_token nor an error".to_string(),
                )
            })?;
        Ok(TokenPoll::Granted(Box::new(GrantedToken {
            access_token: access_token.to_string(),
            id_token: value
                .get("id_token")
                .and_then(Value::as_str)
                .map(str::to_string),
            expires_in: value.get("expires_in").and_then(Value::as_u64),
        })))
    }
}

/// Render a provider error with its description when there is one.
fn describe(error: &str, value: &Value) -> String {
    match value.get("error_description").and_then(Value::as_str) {
        Some(description) => format!("provider refused the device grant: {error} — {description}"),
        None => format!("provider refused the device grant: {error}"),
    }
}

#[cfg(test)]
#[path = "token_poll.tests.rs"]
mod tests;
