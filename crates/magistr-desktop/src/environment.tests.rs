use super::*;

#[test]
fn restores_shell_paths_without_replacing_explicit_commands() {
    let path = merge_path(
        Some(OsStr::new("/fixtures/bin:/usr/bin")),
        b"shell banner\n\0/Users/person/.nvm/node/bin:/usr/bin:/Users/person/.local/bin\n\0bye",
    )
    .unwrap();
    assert_eq!(
        path,
        "/fixtures/bin:/usr/bin:/Users/person/.nvm/node/bin:/Users/person/.local/bin"
    );
    for output in [b"banner only".as_slice(), b"\0\n\0", b"\0incomplete"] {
        assert!(merge_path(None, output).is_err());
    }
}

#[test]
fn command_path_preserves_spaces_and_non_utf8_bytes() {
    let path = merge_path(None, b"\0/Users/a name/bin:/tools/\xff:\n\0").unwrap();
    assert_eq!(path.into_vec(), b"/Users/a name/bin:/tools/\xff");
}

#[test]
fn shell_probe_reads_only_path_and_does_not_change_the_host() {
    let before = std::env::var_os("PATH");
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'banner\\n\\0%s\\n\\0' \"$PATH\""]);
    command.env("PATH", "/fixture/codex/bin:/usr/bin");
    let output = shell_output(command, Duration::from_secs(1)).unwrap();
    assert_eq!(
        merge_path(None, &output).unwrap(),
        "/fixture/codex/bin:/usr/bin"
    );
    assert_eq!(std::env::var_os("PATH"), before);
}

#[test]
fn shell_failure_stall_and_excessive_output_are_bounded() {
    for script in [
        "exit 7",
        "exec /bin/sleep 10",
        "while :; do printf '0123456789'; done",
    ] {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        let start = Instant::now();
        assert!(shell_output(command, Duration::from_millis(100)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
#[test]
fn a_gui_path_discovers_agents_from_login_shell_configuration() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let commands = root.path().join("agent commands");
    std::fs::create_dir(&commands).unwrap();
    let agent = commands.join("codex");
    std::fs::write(&agent, "#!/bin/sh\nprintf 'fixture Codex'\n").unwrap();
    std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        root.path().join(".zshrc"),
        "export PATH=\"$ZDOTDIR/agent commands:$PATH\"\n",
    )
    .unwrap();
    let mut shell = Command::new("/bin/zsh");
    shell
        .args(["-ilc", PATH_COMMAND])
        .env("ZDOTDIR", root.path())
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    let output = shell_output(shell, Duration::from_secs(2)).unwrap();
    let path = merge_path(Some(OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin")), &output).unwrap();
    let result = Command::new("/usr/bin/env")
        .arg("codex")
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"fixture Codex");
}
