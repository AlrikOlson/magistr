/// Cache failures preserve operational errors instead of discarding data.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("workspace index I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("workspace index SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid workspace index input: {0}")]
    InvalidInput(String),
}
