use core::fmt;

/// Invalid externally allocated identity component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    Empty,
    ControlCharacter,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "identity must not be empty",
            Self::ControlCharacter => "identity must not contain control characters",
        })
    }
}

impl core::error::Error for IdentityError {}
