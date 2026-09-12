use std::path::PathBuf;

/// What an explicit offline migration of `server.lock` found and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyMigration {
    /// No lock file: a fresh workspace needs no migration.
    Absent,
    /// The marker is already the native one; nothing was touched.
    AlreadyNative,
    /// The obsolete record was moved aside; the native owner can admit now.
    Migrated { backup: PathBuf },
}
