//! One exclusive OS lock per file, with the file's bytes readable by others.

use std::fs::File;
use std::io;

/// Take the exclusive OS lock; `WouldBlock` when another owner holds it.
/// Unix locks the whole file advisorily, so readers are unaffected.
#[cfg(unix)]
pub(crate) fn try_lock_exclusive(file: &File) -> io::Result<()> {
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
        std::fs::TryLockError::Error(error) => error,
    })
}

#[cfg(unix)]
pub(crate) fn unlock(file: &File) -> io::Result<()> {
    file.unlock()
}

/// Windows locks are mandatory: a whole-file lock would deny every other
/// handle's read of the file, breaking legacy clients and diagnostics that
/// only look. Lock one byte far beyond any content instead: contention is
/// still decided by the OS on one range per file, and the bytes stay readable.
#[cfg(windows)]
const LOCK_OFFSET: u64 = 1 << 62;

#[cfg(windows)]
fn lock_range() -> windows_sys::Win32::System::IO::OVERLAPPED {
    use windows_sys::Win32::System::IO::{OVERLAPPED, OVERLAPPED_0, OVERLAPPED_0_0};
    OVERLAPPED {
        Internal: 0,
        InternalHigh: 0,
        Anonymous: OVERLAPPED_0 {
            Anonymous: OVERLAPPED_0_0 {
                Offset: LOCK_OFFSET as u32,
                OffsetHigh: (LOCK_OFFSET >> 32) as u32,
            },
        },
        hEvent: std::ptr::null_mut(),
    }
}

#[cfg(windows)]
pub(crate) fn try_lock_exclusive(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
    };
    let mut range = lock_range();
    // SAFETY: the handle stays open for the borrow of `file`, and the
    // OVERLAPPED outlives this synchronous call (no event, no completion).
    let locked = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut range,
        )
    };
    if locked != 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
        return Err(io::Error::from(io::ErrorKind::WouldBlock));
    }
    Err(error)
}

#[cfg(windows)]
pub(crate) fn unlock(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::UnlockFileEx;
    let mut range = lock_range();
    // SAFETY: same handle and range as `try_lock_exclusive`; the OVERLAPPED
    // outlives this synchronous call.
    if unsafe { UnlockFileEx(file.as_raw_handle(), 0, 1, 0, &mut range) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
#[path = "os_file_lock.tests.rs"]
mod tests;
