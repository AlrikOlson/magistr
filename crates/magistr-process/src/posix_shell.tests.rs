use super::*;

use std::ffi::OsString;

/// A directory holding the given empty files, standing in for a PATH entry.
fn dir_with(parent: &Path, name: &str, files: &[&str]) -> PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).expect("dir");
    for file in files {
        std::fs::write(dir.join(file), b"").expect("file");
    }
    dir
}

fn joined(dirs: &[&Path]) -> OsString {
    std::env::join_paths(dirs).expect("joinable")
}

#[test]
fn the_first_real_sh_on_path_is_the_shell() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let empty = dir_with(tmp.path(), "empty", &[]);
    let git = dir_with(tmp.path(), "git-usr-bin", &["sh.exe", "bash.exe"]);
    let path = joined(&[&empty, &git]);
    let shell = PosixShell::locate(Some(&path), None).expect("found");
    assert_eq!(shell.sh(), git.join("sh.exe"));
    assert_eq!(shell.bash(), git.join("bash.exe"));
}

#[test]
fn wsl_launcher_directories_are_never_the_shell() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let system = dir_with(tmp.path(), "Windows", &["sh.exe", "bash.exe"]);
    let system32 = dir_with(&system, "System32", &["sh.exe", "bash.exe"]);
    let apps = dir_with(tmp.path(), "WindowsApps", &["sh.exe", "bash.exe"]);
    let path = joined(&[&system32, &apps]);
    assert!(
        PosixShell::locate(Some(&path), Some(system.as_os_str())).is_none(),
        "the system directory and WindowsApps hold the WSL launchers"
    );
}

#[test]
fn a_shell_without_bash_beside_it_answers_bash_with_sh() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let only_sh = dir_with(tmp.path(), "only-sh", &["sh.exe"]);
    let shell = PosixShell::locate(Some(&joined(&[&only_sh])), None).expect("found");
    assert_eq!(shell.bash(), only_sh.join("sh.exe"));
}

#[test]
fn the_childs_path_leads_with_the_shells_directory_once() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let system32 = dir_with(tmp.path(), "System32", &["bash.exe"]);
    let git = dir_with(tmp.path(), "git-usr-bin", &["sh.exe"]);
    let path = joined(&[&system32, &git]);
    let shell = PosixShell::locate(Some(&path), None).expect("found");
    let child: Vec<PathBuf> = std::env::split_paths(&shell.path_for_child(Some(&path))).collect();
    assert_eq!(child, vec![git.clone(), system32]);
}

#[test]
fn only_bare_shell_names_are_shells() {
    for name in ["bash", "sh", "BASH", "bash.exe", "Sh.EXE"] {
        assert!(names_posix_shell(name), "{name}");
    }
    for name in [
        "cargo",
        "zsh",
        r"C:\Windows\System32\bash.exe",
        "/usr/bin/bash",
        "bashful",
    ] {
        assert!(!names_posix_shell(name), "{name}");
    }
}

#[test]
fn a_line_is_led_by_its_first_word() {
    assert!(line_starts_with_posix_shell("bash ./scripts/gate.sh"));
    assert!(line_starts_with_posix_shell("  sh -c 'echo hi'"));
    assert!(!line_starts_with_posix_shell("cargo test"));
    assert!(!line_starts_with_posix_shell(""));
}
