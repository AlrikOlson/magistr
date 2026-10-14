//! Observation state and its provenance travel together so a retained value
//! cannot become current merely because a later read failed.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::StoreStamp;

/// The source of a successful observation of one workflow family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "stamp", rename_all = "snake_case")]
pub enum ReadSource {
    Memory,
    Missing,
    File(StoreStamp),
}

/// Validated data and the exact source that supplied it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Observed<T> {
    pub value: T,
    pub source: ReadSource,
}

impl<T> Observed<T> {
    #[must_use]
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Observed<U> {
        Observed {
            value: map(self.value),
            source: self.source,
        }
    }
}

/// The kind of refusal, separate from its plain recovery message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadErrorKind {
    Io,
    Decode,
    Schema,
    Identity,
    Interrupted,
    Changed,
    Unobserved,
}

/// Public read failure. Detailed paths and parser diagnostics belong in logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadError {
    pub kind: ReadErrorKind,
    pub message: String,
}

impl ReadError {
    #[must_use]
    pub fn new(kind: ReadErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ReadError {}

/// A failed observation retains evidence without certifying it as current.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", content = "data", rename_all = "snake_case")]
pub enum ReadState<T> {
    #[default]
    Unobserved,
    Current(Observed<T>),
    Stale {
        last: Observed<T>,
        error: ReadError,
    },
    Unavailable(ReadError),
}

impl<T> ReadState<T> {
    /// Decisions may only use data from the latest successful read.
    pub fn current(&self) -> Result<&Observed<T>, ReadError> {
        match self {
            Self::Current(observed) => Ok(observed),
            Self::Stale { error, .. } | Self::Unavailable(error) => Err(error.clone()),
            Self::Unobserved => Err(ReadError::new(
                ReadErrorKind::Unobserved,
                "This information has not been read yet. Try again.",
            )),
        }
    }

    /// Retained data for a view that also presents its observation state.
    #[must_use]
    pub fn last_successful(&self) -> Option<&Observed<T>> {
        match self {
            Self::Current(observed) | Self::Stale { last: observed, .. } => Some(observed),
            Self::Unobserved | Self::Unavailable(_) => None,
        }
    }

    #[must_use]
    pub fn needs_retry(&self) -> bool {
        !matches!(self, Self::Current(_))
    }

    /// Apply an observation while retaining only the most recent success.
    pub fn record(&mut self, result: Result<Observed<T>, ReadError>) {
        *self = match result {
            Ok(observed) => Self::Current(observed),
            Err(error) => match std::mem::take(self) {
                Self::Current(last) | Self::Stale { last, .. } => Self::Stale { last, error },
                Self::Unobserved | Self::Unavailable(_) => Self::Unavailable(error),
            },
        };
    }

    /// Derive a view without dropping freshness or its original provenance.
    #[must_use]
    pub fn map<U>(&self, map: impl FnOnce(&T) -> U) -> ReadState<U> {
        match self {
            Self::Unobserved => ReadState::Unobserved,
            Self::Current(observed) => ReadState::Current(Observed {
                value: map(&observed.value),
                source: observed.source,
            }),
            Self::Stale { last, error } => ReadState::Stale {
                last: Observed {
                    value: map(&last.value),
                    source: last.source,
                },
                error: error.clone(),
            },
            Self::Unavailable(error) => ReadState::Unavailable(error.clone()),
        }
    }
}

#[cfg(test)]
#[path = "read_state.tests.rs"]
mod tests;
