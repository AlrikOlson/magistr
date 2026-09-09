//! Result type alias for engine client operations.

use super::error::EngineClientError;

/// Result type alias for engine client operations.
pub type Result<T> = std::result::Result<T, EngineClientError>;
