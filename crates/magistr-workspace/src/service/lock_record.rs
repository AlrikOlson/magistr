/// What `.magistr/workspace/server.lock` holds, read without taking the lock. A
/// record is bytes on disk: the native marker says only that a native owner
/// once admitted this workspace (ownership is the OS lock, never the
/// marker). Unknown records are not interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockRecord {
    /// No lock file: no owner was ever admitted here.
    Absent,
    /// The immutable native marker: native owners admit through the OS lock
    /// on this inode. Whether one holds it now is not readable here.
    Native,
    /// Empty or unrecognized bytes; admission is refused.
    Unrecognized,
}
