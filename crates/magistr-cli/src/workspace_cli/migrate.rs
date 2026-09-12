//! `magistr workspace migrate`: the documented offline migration of an
//! obsolete `server.lock` written by the retained legacy server (or an empty
//! pre-migration lock), as one command that refuses when it cannot be sure.

use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::service::{LegacyMigration, ServerLock, WorkspaceOpenError};

use super::admitted_root;

/// Move the obsolete lock record aside, or say why not (exit 75).
pub fn run_migrate(dir: &Path) -> eyre::Result<()> {
    let root = admitted_root(dir);
    let lock = root.join(".workbench").join(ServerLock::FILE_NAME);
    match ServerLock::migrate_legacy(&lock) {
        Ok(LegacyMigration::Absent) => {
            eprintln!(
                "magistr workspace: no lock record at {} — nothing to migrate",
                lock.display()
            );
        }
        Ok(LegacyMigration::AlreadyNative) => {
            eprintln!(
                "magistr workspace: {} is already the native marker — nothing to migrate",
                lock.display()
            );
        }
        Ok(LegacyMigration::Migrated { backup }) => {
            eprintln!(
                "magistr workspace: moved the legacy lock record to {}; the native owner can admit this workspace now",
                backup.display()
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            super::fail(WorkspaceOpenError::EXIT_LOCKED, &error.to_string());
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
