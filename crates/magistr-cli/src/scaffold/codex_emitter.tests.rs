#[cfg(unix)]
use super::*;

#[test]
#[cfg(unix)]
fn launcher_preserves_arguments_and_resolves_its_project() {
    use std::os::unix::fs::PermissionsExt;

    let files = CodexEmitter.files(&HashMap::new());
    let launcher = files.iter().find(|file| file.path == ".codex/run").unwrap();
    assert!(launcher.executable);
    assert_eq!(launcher.policy, UpdatePolicy::PreserveCustomized);
    let tmp = tempfile::TempDir::new().unwrap();
    let project = tmp.path().join("project ' with $ literals");
    std::fs::create_dir_all(project.join(".codex")).unwrap();
    std::fs::write(project.join(launcher.path), &launcher.content).unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let fake = bin.join("magistr");
    std::fs::write(
        &fake,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$MAGISTR_TEST_ARGS\"\n",
    )
    .unwrap();
    std::fs::set_permissions(fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let captured = tmp.path().join("args");
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let status = std::process::Command::new("sh")
        .arg(project.join(launcher.path))
        .args(["--model", "literal $(touch nope); value", ""])
        .current_dir(tmp.path())
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("MAGISTR_TEST_ARGS", &captured)
        .status()
        .unwrap();
    assert!(status.success());
    let captured = std::fs::read_to_string(captured).unwrap();
    let argv: Vec<_> = captured.lines().collect();
    assert_eq!(&argv[..2], ["codex-launch", "--project-root"]);
    assert_eq!(
        std::path::Path::new(argv[2]).canonicalize().unwrap(),
        project.canonicalize().unwrap()
    );
    assert_eq!(
        &argv[3..],
        ["--", "--model", "literal $(touch nope); value", ""]
    );
    assert!(!tmp.path().join("nope").exists());
}
