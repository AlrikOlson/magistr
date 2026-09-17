/// What `.workbench/server.lock` holds, read without taking the lock. A
/// record is bytes on disk: the native marker says only that a native owner
/// once admitted this workspace (ownership is the OS lock, never the
/// marker), and a legacy record's pid is at most a hint about a process
/// that may since have ended or been reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockRecord {
    /// No lock file: no owner was ever admitted here.
    Absent,
    /// The immutable native marker: native owners admit through the OS lock
    /// on this inode. Whether one holds it now is not readable here.
    Native,
    /// The retained legacy server's record, which needs the offline
    /// migration before a native owner can admit.
    Legacy {
        /// The host the record names.
        host: String,
        /// The process the record names.
        pid: u64,
        /// Whether that pid is alive on this host: `None` when the record
        /// names another host (nothing here can judge it) or no pid.
        pid_alive: Option<bool>,
    },
    /// Empty or unparseable bytes: the migration treats it as obsolete.
    Unrecognized,
}
