use super::cache::CacheError;
/// Failures of the derived cache capability; no durable state is affected.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("index cache: {0}")]
    Cache(#[from] CacheError),
    #[error("index I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("index unavailable: {0}")]
    Unavailable(String),
}
