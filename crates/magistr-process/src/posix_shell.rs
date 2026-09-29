//! The POSIX shell a Windows launch runs under, and never WSL.
//!
//! Windows ships `bash.exe` launchers for WSL in the system directory and in
//! `WindowsApps`. A child that resolves `bash` through an ordinary Windows
//! PATH — a gate line `bash ./scripts/x.sh`, a script's
//! `#!/usr/bin/env bash`, an agent's `bash -c` — can land in a different
//! operating system with a different toolchain, or hang while WSL starts.
//! These items decide together which shell is real (a `sh.exe` outside
//! those directories: Git for Windows'), which names mean "that shell", and
//! what PATH its children see so every nested `bash` resolves beside it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// A POSIX shell found on PATH outside the directories holding the WSL
/// launchers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PosixShell {
    dir: PathBuf,
}

impl PosixShell {
    /// The first directory on `path` holding `sh.exe` that is neither under
    /// `system_root` (the Windows directory) nor a `WindowsApps` directory.
    pub(crate) fn locate(path: Option<&OsStr>, system_root: Option<&OsStr>) -> Option<Self> {
        let system_root = system_root.map(Path::new);
        std::env::split_paths(path?)
            .filter(|dir| !is_wsl_launcher_dir(dir, system_root))
            .find(|dir| dir.join("sh.exe").is_file())
            .map(|dir| Self { dir })
    }

    /// The shell a POSIX line runs under.
    pub(crate) fn sh(&self) -> PathBuf {
        self.dir.join("sh.exe")
    }

    /// What `bash` means for a launch: the bash beside the shell, or the
    /// shell itself when there is none.
    pub(crate) fn bash(&self) -> PathBuf {
        let bash = self.dir.join("bash.exe");
        if bash.is_file() { bash } else { self.sh() }
    }

    /// `path` with the shell's directory first and not repeated, so a
    /// nested `bash`, `sh` or `env bash` resolves beside the shell.
    pub(crate) fn path_for_child(&self, path: Option<&OsStr>) -> OsString {
        let rest = path
            .map(std::env::split_paths)
            .into_iter()
            .flatten()
            .filter(|dir| dir != &self.dir);
        std::env::join_paths(std::iter::once(self.dir.clone()).chain(rest))
            .unwrap_or_else(|_| self.dir.clone().into_os_string())
    }
}

/// Whether `dir` is where Windows keeps the WSL launchers.
fn is_wsl_launcher_dir(dir: &Path, system_root: Option<&Path>) -> bool {
    let under_root = system_root.is_some_and(|root| {
        dir.to_string_lossy()
            .to_lowercase()
            .starts_with(&root.to_string_lossy().to_lowercase())
    });
    let windows_apps = dir
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("WindowsApps"));
    under_root || windows_apps
}

/// Whether `program` is a bare `bash` or `sh` — a name PATH would resolve,
/// not a path the caller chose.
pub(crate) fn names_posix_shell(program: &str) -> bool {
    let bare = !program.contains(['/', '\\']);
    let stem = program
        .strip_suffix(".exe")
        .or_else(|| program.strip_suffix(".EXE"))
        .unwrap_or(program);
    bare && (stem.eq_ignore_ascii_case("bash") || stem.eq_ignore_ascii_case("sh"))
}

/// Whether a shell line's first word is a bare `bash` or `sh`.
pub(crate) fn line_starts_with_posix_shell(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(names_posix_shell)
}

#[cfg(test)]
#[path = "posix_shell.tests.rs"]
mod tests;
