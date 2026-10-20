//! Runtime boundary checks use a local executable fixture; no agent turn runs.
#![cfg(unix)]

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use magistr::VERIFIED_CODEX_VERSION;

struct Fixture {
    _temp: tempfile::TempDir,
    project: PathBuf,
    codex_home: PathBuf,
    home: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new(script: &str) -> Self {
        let temp = tempfile::tempdir().expect("temporary runtime");
        let root = temp.path().canonicalize().expect("canonical fixture");
        let project = root.join("project space");
        let codex_home = root.join("custom config");
        let home = root.join("personal home");
        let bin = root.join("bin");
        for path in [&project, &codex_home, &home, &bin] {
            std::fs::create_dir_all(path).expect("fixture directory");
        }
        std::fs::create_dir(project.join(".git")).expect("repository marker");
        std::fs::write(bin.join("codex"), script).expect("fake codex");
        std::fs::set_permissions(bin.join("codex"), std::fs::Permissions::from_mode(0o755))
            .expect("executable fake codex");
        Self {
            _temp: temp,
            project,
            codex_home,
            home,
            bin,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::from_std(magistr_command::magistr_command());
        command
            .env("PATH", &self.bin)
            .env("HOME", &self.home)
            .env("CODEX_HOME", &self.codex_home)
            .arg("codex-launch")
            .arg("--project-root")
            .arg(&self.project);
        command
    }

    fn skill(&self, root: &Path, folder: &str, name: &str) -> PathBuf {
        let skill = root.join(folder).join("SKILL.md");
        std::fs::create_dir_all(skill.parent().expect("parent")).expect("skill directory");
        std::fs::write(
            &skill,
            format!("---\nname: {name}\ndescription: Fixture.\n---\n"),
        )
        .expect("skill file");
        skill
    }
}

#[test]
fn native_codex_launch_preserves_argv_stdio_environment_and_exit_status() {
    let f = Fixture::new("#!/bin/sh\nprintf '%s\\0' \"$PWD\" \"$CODEX_HOME\" \"$@\"\nexit 37\n");
    let literal = OsString::from_vec(b"literal\xff$(touch never); `echo nope`".to_vec());
    let output = f
        .command()
        .arg("--")
        .args([OsString::from("--"), literal.clone()])
        .assert()
        .code(37)
        .get_output()
        .stdout
        .clone();
    let fields: Vec<_> = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|bytes| OsString::from_vec(bytes.to_vec()))
        .collect();
    assert_eq!(
        fields,
        vec![
            f.project.clone().into_os_string(),
            f.codex_home.into_os_string(),
            OsString::from("-C"),
            f.project.into_os_string(),
            OsString::from("--"),
            literal
        ]
    );
}

#[test]
fn diagnostic_reports_known_version_and_project_skill_override_without_launching_a_turn() {
    let f = Fixture::new(&format!(
        "#!/bin/sh\n[ \"$1\" = '--version' ] || exit 99\nprintf 'codex-cli {VERIFIED_CODEX_VERSION}\\n'\n",
    ));
    f.skill(&f.project.join(".agents/skills"), "next", "next");
    let duplicate = f.skill(&f.codex_home.join("skills"), "legacy-next", "next");
    let output = f
        .command()
        .arg("--diagnose")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).expect("JSON diagnosis");
    assert_eq!(report["matches_verified_version"], true);
    assert_eq!(report["working_root"], f.project.to_str().expect("UTF8"));
    assert_eq!(report["codex_home"], f.codex_home.to_str().expect("UTF8"));
    assert_eq!(
        report["suppressed_personal_skills"],
        serde_json::json!([duplicate])
    );
}

#[test]
fn diagnostic_distinguishes_unverified_cli_versions() {
    let f = Fixture::new(
        "#!/bin/sh\n[ \"$1\" = '--version' ] || exit 99\nprintf 'codex-cli 0.0.0-unverified\\n'\n",
    );
    let output = f
        .command()
        .arg("--diagnose")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).expect("JSON diagnosis");
    assert_eq!(report["matches_verified_version"], false);
    assert_eq!(report["verified_version"], VERIFIED_CODEX_VERSION);
}

#[test]
fn forwarded_cd_uses_wrapper_root_and_does_not_duplicate_cd_flag() {
    let f = Fixture::new("#!/bin/sh\nprintf '%s\\0' \"$PWD\" \"$@\"\n");
    std::fs::create_dir(f.project.join("nested")).expect("nested directory");
    let output = f
        .command()
        .args(["--", "-C", "nested", "--", "a prompt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let fields: Vec<_> = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|bytes| OsString::from_vec(bytes.to_vec()))
        .collect();
    assert_eq!(
        fields,
        vec![
            f.project.into_os_string(),
            OsString::from("-C"),
            OsString::from("nested"),
            OsString::from("--"),
            OsString::from("a prompt")
        ]
    );
}

#[test]
fn missing_codex_reports_install_guidance_before_any_runtime_fallback() {
    let f = Fixture::new("#!/bin/sh\nexit 0\n");
    std::fs::remove_file(f.bin.join("codex")).expect("remove fake runtime");
    let output = f
        .command()
        .arg("--diagnose")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(String::from_utf8_lossy(&output).contains("Codex CLI is not on PATH"));
}
