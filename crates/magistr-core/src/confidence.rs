//! Confidence level for detection results (0–100).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Confidence level for a detection result (0–100).
///
/// Ranges:
/// - 90–100: Definitive marker (e.g., `Cargo.toml` → Rust)
/// - 60–89: Strong indirect signal (e.g., dependency in manifest)
/// - 30–59: Heuristic / weak signal (e.g., file extensions only)
/// - 0–29: No meaningful signal, or `Unknown`/`None` variant
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Confidence(u8);

impl Confidence {
    /// Create a confidence score, clamping to 0–100.
    pub fn new(score: u8) -> Self {
        Self(score.min(100))
    }

    /// No confidence at all (0).
    pub fn none() -> Self {
        Self(0)
    }

    /// High confidence from a definitive marker (95).
    pub fn high() -> Self {
        Self(95)
    }

    /// Medium confidence from indirect signals (70).
    pub fn medium() -> Self {
        Self(70)
    }

    /// Low confidence from heuristics (40).
    pub fn low() -> Self {
        Self(40)
    }

    /// Get the raw score.
    pub fn score(self) -> u8 {
        self.0
    }

    /// Whether this is considered low confidence (below 60).
    pub fn is_low(self) -> bool {
        self.0 < 60
    }

    /// Whether this is considered high confidence (90+).
    pub fn is_high(self) -> bool {
        self.0 >= 90
    }

    /// Human-readable label for the confidence level.
    pub fn label(self) -> &'static str {
        match self.0 {
            90..=100 => "high",
            60..=89 => "medium",
            30..=59 => "low",
            _ => "none",
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}
