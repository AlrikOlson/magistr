//! Who a Windows process or pipe peer runs as, and the owner-only security
//! descriptor an endpoint's pipe is created with.
//!
//! These decide together what "the owner's user" means on Windows: the
//! string SID of a token's user. The pipe grants that SID alone, the owner
//! checks each connecting client's token against it, and a client checks the
//! serving process against it before trusting a greeting. They are the
//! Windows counterpart of the Unix socket's mode 0600 and `peer_cred` uid.

use std::{io, path::Path, ptr};

use tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, PSID, RevertToSelf, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeServerProcessId, ImpersonateNamedPipeClient};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, OpenProcess, OpenProcessToken, OpenThreadToken,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

/// A kernel handle this module opened, closed on drop.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was returned open by the call that made this
        // value and is closed exactly once, here.
        unsafe { CloseHandle(self.0) };
    }
}

/// The user this process runs as.
///
/// # Errors
/// The process token cannot be opened or read.
pub(super) fn current_user() -> io::Result<String> {
    let mut token = ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no
    // closing; `token` is a valid out pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    token_user(&OwnedHandle(token))
}

/// The user a connected client of `server` runs as, read from its own
/// token by identification-level impersonation — never from a process id,
/// which a client can misreport. At least one byte must already have been
/// read from the client; the handshake line always has been.
///
/// # Errors
/// The client's token cannot be read.
pub(super) fn client_user(server: &NamedPipeServer) -> io::Result<String> {
    use std::os::windows::io::AsRawHandle;
    let pipe = server.as_raw_handle();
    // SAFETY: `pipe` is the live handle `server` owns for this call.
    if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut token = ptr::null_mut();
    // SAFETY: the pseudo-handle names this thread, which is impersonating;
    // opening as self checks access against this process, not the client.
    let opened = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) };
    let open_error = io::Error::last_os_error();
    // SAFETY: RevertToSelf takes no arguments and ends this thread's
    // impersonation before anything else runs on it.
    if unsafe { RevertToSelf() } == 0 {
        // A worker thread left running as another user must not execute
        // anything more; the platform's guidance is to end the process.
        std::process::abort();
    }
    if opened == 0 {
        return Err(open_error);
    }
    token_user(&OwnedHandle(token))
}

/// The user the process serving `client`'s pipe runs as.
///
/// # Errors
/// The server's process or token cannot be opened, which is the case for
/// another user's process.
pub(super) fn server_user(client: &NamedPipeClient) -> io::Result<String> {
    use std::os::windows::io::AsRawHandle;
    let mut pid = 0_u32;
    // SAFETY: the handle is live for the call and `pid` is a valid out pointer.
    if unsafe { GetNamedPipeServerProcessId(client.as_raw_handle(), &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcess has no memory preconditions; a null return is checked.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = OwnedHandle(process);
    let mut token = ptr::null_mut();
    // SAFETY: `process` is open with query rights; `token` is a valid out pointer.
    if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    token_user(&OwnedHandle(token))
}

fn token_user(token: &OwnedHandle) -> io::Result<String> {
    let mut needed = 0_u32;
    // SAFETY: a null buffer of length zero asks only for the required size.
    unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut needed) };
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    // u64 elements keep the buffer aligned for TOKEN_USER.
    let mut buffer = vec![0_u64; (needed as usize).div_ceil(8)];
    // SAFETY: the buffer holds at least `needed` bytes and is 8-aligned.
    let read = unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success the buffer starts with a TOKEN_USER whose SID
    // points inside the same buffer, which outlives this read.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    sid_string(sid)
}

fn sid_string(sid: PSID) -> io::Result<String> {
    let mut text = ptr::null_mut();
    // SAFETY: `sid` is a valid SID for the call; `text` is a valid out pointer.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success `text` is a NUL-terminated LocalAlloc'd string.
    Ok(unsafe { take_local_wide(text) })
}

/// Copy a NUL-terminated wide string the system allocated with LocalAlloc,
/// then free it.
///
/// # Safety
/// `text` is non-null, NUL-terminated and owned by the caller.
unsafe fn take_local_wide(text: *mut u16) -> String {
    let mut len = 0;
    // SAFETY: the caller guarantees a terminator within the allocation.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` elements before the terminator are initialized.
    let value = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
    // SAFETY: the allocation came from LocalAlloc and is freed once.
    unsafe { LocalFree(text.cast()) };
    value
}

/// A self-relative descriptor that grants one user every right and nobody
/// else anything: no inherited entries, no Everyone, no anonymous. Its
/// owner is that user too, so no group owner holds implicit rights.
pub(super) struct OwnerOnlyDescriptor(PSECURITY_DESCRIPTOR);

// SAFETY: the descriptor is a private, immutable LocalAlloc block; Windows
// only reads it, and only while this value is alive.
unsafe impl Send for OwnerOnlyDescriptor {}

impl OwnerOnlyDescriptor {
    /// The descriptor for `user`, a string SID.
    ///
    /// # Errors
    /// The SID is not well formed.
    pub(super) fn for_user(user: &str) -> io::Result<Self> {
        let sddl: Vec<u16> = format!("O:{user}D:P(A;;GA;;;{user})")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated and outlives the call; the size
        // out pointer is optional.
        let made = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        };
        if made == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }

    /// One instance of the pipe `name` under this descriptor, rejecting
    /// remote clients. `first` claims the name: it fails when any instance
    /// already exists, whoever made it.
    ///
    /// # Errors
    /// As `CreateNamedPipeW`; a claimed name is `PermissionDenied`.
    pub(super) fn create(&self, name: &Path, first: bool) -> io::Result<NamedPipeServer> {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).map_err(io::Error::other)?,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        };
        // SAFETY: `attributes` points at this live descriptor for the whole
        // call; CreateNamedPipeW copies what it needs.
        unsafe {
            ServerOptions::new()
                .first_pipe_instance(first)
                .reject_remote_clients(true)
                .create_with_security_attributes_raw(name, (&raw mut attributes).cast())
        }
    }
}

impl Drop for OwnerOnlyDescriptor {
    fn drop(&mut self) {
        // SAFETY: the descriptor came from LocalAlloc and is freed once.
        unsafe { LocalFree(self.0) };
    }
}

/// The DACL of the pipe `client` is connected to, in SDDL — what a test or
/// a diagnostic reads to see who may open it.
///
/// # Errors
/// The pipe's security cannot be read.
#[cfg(test)]
pub(super) fn pipe_dacl(client: &NamedPipeClient) -> io::Result<String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SE_KERNEL_OBJECT,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
    let mut descriptor = ptr::null_mut();
    // SAFETY: the handle is live; only the descriptor out pointer is requested.
    let status = unsafe {
        GetSecurityInfo(
            client.as_raw_handle(),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status.cast_signed()));
    }
    let mut text = ptr::null_mut();
    // SAFETY: `descriptor` was just returned by GetSecurityInfo.
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            ptr::null_mut(),
        )
    };
    let error = io::Error::last_os_error();
    // SAFETY: GetSecurityInfo's descriptor is LocalAlloc'd and freed once.
    unsafe { LocalFree(descriptor) };
    if converted == 0 {
        return Err(error);
    }
    // SAFETY: on success `text` is a NUL-terminated LocalAlloc'd string.
    Ok(unsafe { take_local_wide(text) })
}

#[cfg(test)]
#[path = "user_sid.tests.rs"]
mod tests;
