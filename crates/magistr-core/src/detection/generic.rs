//! A detected value with confidence score and human-readable evidence.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::confidence::Confidence;

/// A detected value with confidence score and human-readable evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection<T> {
    /// The detected value.
    pub value: T,
    /// How confident we are in this detection (0–100).
    pub confidence: Confidence,
    /// Human-readable reasons for this detection.
    pub evidence: Vec<String>,
}

impl<T> Detection<T> {
    /// Create a detection with a single evidence string.
    pub fn new(value: T, confidence: Confidence, evidence: impl Into<String>) -> Self {
        Self {
            value,
            confidence,
            evidence: vec![evidence.into()],
        }
    }

    /// Create a detection with multiple evidence strings.
    pub fn with_evidence(value: T, confidence: Confidence, evidence: Vec<String>) -> Self {
        Self {
            value,
            confidence,
            evidence,
        }
    }

    /// Create a detection with no signal (zero confidence, no evidence).
    pub fn none(value: T) -> Self {
        Self {
            value,
            confidence: Confidence::none(),
            evidence: vec![],
        }
    }

    /// Whether the detection has low confidence and should be flagged.
    pub fn needs_override(&self) -> bool {
        self.confidence.is_low()
    }
}

impl<T: fmt::Display> fmt::Display for Detection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.value, self.confidence)
    }
}
