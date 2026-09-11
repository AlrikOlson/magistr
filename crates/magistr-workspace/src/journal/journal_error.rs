/// Durable journal failures; operational errors never cause quarantine.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("journal I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("journal SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("UNSUPPORTED_VERSION: {0}")]
    UnsupportedVersion(String),
    #[error("invalid journal input: {0}")]
    InvalidInput(String),
    #[error("journal read limit exceeded: {0}")]
    ReadLimitExceeded(String),
    #[error("corrupt journal blob {hash}: {reason}")]
    CorruptBlob { hash: String, reason: String },
    #[error("corrupt journal record: {0}")]
    CorruptRecord(String),
    #[error(
        "journal recovery saved durable data at {quarantined_to}, but creating the replacement failed: {source}"
    )]
    RecoveryFailed {
        quarantined_to: std::path::PathBuf,
        #[source]
        source: Box<JournalError>,
    },
}
