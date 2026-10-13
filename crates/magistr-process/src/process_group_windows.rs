//! Windows command ownership: contain the suspended child before it can fork.
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::time::Duration;

use tokio::process::Command;
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

use crate::{ChildRegistry, ReapingChild};

pub(crate) const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

/// The OS job containing one command and every process it creates.
/// Its handle is never inherited; abrupt host exit closes it and kills the job.
pub struct ProcessGroup {
    job: OwnedHandle,
    pid: Option<i32>,
}

impl ProcessGroup {
    /// Spawn suspended, assign the job, then resume the child's primary thread.
    /// No user code runs before containment; a failed step kills/reaps the child.
    pub fn spawn(command: &mut Command) -> io::Result<(ReapingChild, Self)> {
        let mut group = Self::new()?;
        command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        let child = ReapingChild::new(command.spawn()?);
        let pid = child
            .id()
            .ok_or_else(|| io::Error::other("missing child pid"))?;
        let process = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("missing child handle"))?;
        // SAFETY: both handles are live; the new child has not executed yet.
        checked(unsafe { AssignProcessToJobObject(group.job.as_raw_handle(), process) })?;
        group.pid = Some(i32::try_from(pid).map_err(io::Error::other)?);
        ChildRegistry::register_job(group.id(), &group.job)?;
        resume_primary_thread(pid)?;
        Ok((child, group))
    }

    fn new() -> io::Result<Self> {
        // SAFETY: null security attributes create a non-inheritable owned handle.
        let job = owned(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })?;
        // SAFETY: all-zero limit information has no pointer invariants.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the job and correctly sized limit structure are live.
        checked(unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        })?;
        Ok(Self { job, pid: None })
    }

    /// The registered command pid, or zero after confirmed settlement.
    #[must_use]
    pub fn id(&self) -> i32 {
        self.pid.unwrap_or(0)
    }

    /// Request termination of every process in the job, including grandchildren.
    pub fn kill(&self) -> io::Result<()> {
        // SAFETY: this handle owns only this command's job.
        checked(unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) })
    }

    /// Confirm the entire job is empty, rather than only the shell's exit.
    pub fn is_settled(&mut self) -> io::Result<bool> {
        // SAFETY: all-zero accounting information is a valid output buffer.
        let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: the job, buffer and exact buffer length are valid.
        checked(unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut info).cast(),
                std::mem::size_of_val(&info) as u32,
                std::ptr::null_mut(),
            )
        })?;
        if info.ActiveProcesses != 0 {
            return Ok(false);
        }
        self.unregister();
        Ok(true)
    }

    /// Wait for confirmed job emptiness under the caller's cleanup deadline.
    pub async fn wait_for_settlement(&mut self, timeout: Duration) -> io::Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        while !self.is_settled()? {
            if tokio::time::Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Windows job cleanup incomplete",
                ));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    fn unregister(&mut self) {
        if let Some(pid) = self.pid.take() {
            ChildRegistry::unregister(pid);
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = self.kill();
        self.unregister();
        // OwnedHandle closes the final job handle even if termination failed.
    }
}

fn owned(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful Win32 creation/open call transfers ownership.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

fn checked(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn resume_primary_thread(pid: u32) -> io::Result<()> {
    // Stable Rust closes CreateProcess's thread handle. The suspended process
    // cannot create another thread; retrieve its primary thread through ToolHelp.
    // SAFETY: snapshot flags have no pointer arguments; ownership is checked.
    let snapshot = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    // SAFETY: THREADENTRY32 is a plain output structure, sized before the call.
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of_val(&entry) as u32;
    // SAFETY: snapshot and sized output structure are valid.
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while found != 0 {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: the suspended child is still owned, preventing pid reuse.
            let thread =
                owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })?;
            // SAFETY: only our suspended child's primary thread is resumed once.
            return if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            };
        }
        // SAFETY: both snapshot and output buffer remain live.
        found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    Err(io::Error::other(
        "suspended child's primary thread was not found",
    ))
}
