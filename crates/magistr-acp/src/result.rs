//! Result alias for transport operations.

/// `Result` with [`AcpError`](crate::AcpError) as the error.
pub type Result<T> = std::result::Result<T, crate::AcpError>;
