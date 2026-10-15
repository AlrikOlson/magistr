//! macOS desktop bootstrap reads the user's command path before any runtime exists.
//! Protocol children inherit the resolved path and never start a login shell.
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::{ffi::OsStringExt, net::UnixStream, process::CommandExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(3);
const PATH_COMMAND: &str = "/usr/bin/printf '\\0'; /usr/bin/printenv PATH; /usr/bin/printf '\\0'";

pub(crate) fn command_path() -> io::Result<OsString> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let mut command = Command::new(shell);
    command.args(["-ilc", PATH_COMMAND]);
    let output = shell_output(command, DEADLINE)?;
    merge_path(std::env::var_os("PATH").as_deref(), &output)
}

fn merge_path(inherited: Option<&OsStr>, output: &[u8]) -> io::Result<OsString> {
    // Startup banners stay outside the NUL-delimited value. Read PATH only;
    // authentication variables and other shell settings are never imported.
    let start = output.iter().position(|byte| *byte == 0);
    let value = start
        .and_then(|start| {
            let tail = &output[start + 1..];
            tail.iter()
                .position(|byte| *byte == 0)
                .map(|end| &tail[..end])
        })
        .ok_or_else(|| io::Error::other("The login shell did not return a command path."))?;
    let value = value.strip_suffix(b"\n").unwrap_or(value);
    if value.is_empty() {
        return Err(io::Error::other(
            "The login shell returned an empty command path.",
        ));
    }
    let recovered = OsString::from_vec(value.to_vec());
    let mut paths = Vec::new();
    // Preserve explicitly supplied commands before appending the shell's paths.
    for path in inherited
        .into_iter()
        .flat_map(std::env::split_paths)
        .chain(std::env::split_paths(&recovered))
    {
        if !path.as_os_str().is_empty() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    std::env::join_paths(paths).map_err(io::Error::other)
}

fn shell_output(mut command: Command, timeout: Duration) -> io::Result<Vec<u8>> {
    // Nonblocking output keeps both chatty and stuck startup files bounded,
    // without spawning a reader thread before the caller installs PATH.
    let (mut reader, writer) = UnixStream::pair()?;
    reader.set_nonblocking(true)?;
    command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .process_group(0);
    let mut child = command.spawn()?;
    drop(command);
    // SAFETY: this child was just spawned in its own process group; this is
    // its only owning guard.
    let mut group = unsafe { magistr_process::ProcessGroup::from_spawned_pid(child.id()) };
    let deadline = Instant::now() + timeout;
    let result = (|| {
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        let mut eof = false;
        loop {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Reading the login shell command path timed out.",
                ));
            }
            match reader.read(&mut buffer) {
                Ok(0) => eof = true,
                Ok(count) => {
                    if bytes.len() + count > LIMIT {
                        return Err(io::Error::other(
                            "The login shell returned too much output.",
                        ));
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(io::Error::other(
                        "The login shell could not read the command path.",
                    ));
                }
                if eof {
                    return Ok(bytes);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    // Settle the shell and its descendants on success as well as failure.
    // Never return a PATH while a cleanup thread might still be running.
    group.kill()?;
    child.wait()?;
    let cleanup_deadline = Instant::now() + Duration::from_millis(500);
    while !group.is_settled()? {
        if Instant::now() >= cleanup_deadline {
            return Err(io::Error::other("The login shell did not finish closing."));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    result
}

#[cfg(test)]
#[path = "environment.tests.rs"]
mod tests;
