use serde::{Deserialize, Serialize};

/// Which side of a completion a token count belongs to.
///
/// The two values OpenTelemetry's `gen_ai.token.type` attribute allows.
/// Cache reads and cache writes have no value of their own in the
/// convention, so Magistr folds them into `Input` at the call site
/// rather than inventing a third variant a collector would not accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    /// Tokens the model consumed.
    Input,
    /// Tokens the model produced.
    Output,
}

impl TokenType {
    /// Both values, input first.
    pub const ALL: [TokenType; 2] = [TokenType::Input, TokenType::Output];

    /// The `gen_ai.token.type` attribute value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            TokenType::Input => "input",
            TokenType::Output => "output",
        }
    }
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "token_type.tests.rs"]
mod tests;
