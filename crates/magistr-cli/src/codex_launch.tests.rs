use super::*;

struct Fixture {
    _temp: tempfile::TempDir,
    project: PathBuf,
    locations: ConfigLocations,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("fixture");
        let root = temp.path().canonicalize().expect("canonical fixture");
        let project = root.join("project space");
        std::fs::create_dir_all(project.join(".git")).expect("project");
        let locations = ConfigLocations {
            codex_home: root.join("custom-codex-home"),
            home: root.join("user-home"),
            system_config: root.join("system/config.toml"),
        };
        Self {
            _temp: temp,
            project,
            locations,
        }
    }

    fn write(&self, path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directories");
        std::fs::write(path, content).expect("fixture file");
    }

    fn skill(&self, root: &Path, folder: &str, name: &str) -> PathBuf {
        let path = root.join(folder).join("SKILL.md");
        self.write(
            &path,
            &format!("---\nname: {name}\ndescription: Test fixture.\n---\nInstructions.\n"),
        );
        path
    }
}

fn selections(plan: &LaunchPlan) -> toml::Table {
    let value = plan
        .args
        .iter()
        .find_map(|arg| {
            arg.to_str()
                .filter(|value| value.starts_with("skills.config="))
        })
        .expect("skill override");
    value.parse().expect("valid TOML override")
}

fn enabled(plan: &LaunchPlan, path: &Path) -> Option<bool> {
    selections(plan)["skills"]["config"]
        .as_array()
        .expect("array")
        .iter()
        .find_map(|entry| {
            (entry["path"].as_str() == path.to_str())
                .then(|| entry["enabled"].as_bool().expect("bool"))
        })
}

#[test]
fn launcher_suppresses_personal_collisions_and_keeps_unrelated_selections() {
    let f = Fixture::new();
    f.skill(&f.project.join(".agents/skills"), "next", "next");
    let duplicate = f.skill(
        &f.locations.codex_home.join("skills"),
        "different-folder",
        "next",
    );
    let untouched = f.skill(&f.locations.home.join(".agents/skills"), "notes", "notes");
    f.write(
        &f.locations.codex_home.join("config.toml"),
        &format!(
            "[[skills.config]]\npath = {}\nenabled = false\n",
            toml::Value::String(untouched.to_str().expect("UTF8").to_owned())
        ),
    );
    let before = std::fs::read(f.locations.codex_home.join("config.toml")).expect("before");
    let plan = build_plan(&f.project, &[], &f.locations).expect("plan");
    assert_eq!(enabled(&plan, &duplicate), Some(false));
    assert_eq!(enabled(&plan, &untouched), Some(false));
    assert_eq!(plan.suppressed, vec![duplicate]);
    assert_eq!(
        std::fs::read(f.locations.codex_home.join("config.toml")).expect("after"),
        before
    );
}

#[test]
fn launcher_applies_nested_layers_and_profile_without_losing_other_paths() {
    let f = Fixture::new();
    let nested = f.project.join("packages/app");
    std::fs::create_dir_all(&nested).expect("nested");
    let skill = f.skill(&f.project.join(".agents/skills"), "custom", "custom");
    let profile_skill = f.skill(&f.locations.codex_home.join("skills"), "profile", "profile");
    f.write(
        &f.project.join(".codex/config.toml"),
        "[[skills.config]]\npath = '../.agents/skills/custom/SKILL.md'\nenabled = false\n",
    );
    f.write(
        &nested.join(".codex/config.toml"),
        &format!(
            "[[skills.config]]\npath = {}\nenabled = true\n",
            toml::Value::String(skill.to_str().expect("UTF8").to_owned())
        ),
    );
    f.write(
        &f.locations.codex_home.join("review.config.toml"),
        &format!(
            "[[skills.config]]\npath = {}\nenabled = false\n",
            toml::Value::String(profile_skill.to_str().expect("UTF8").to_owned())
        ),
    );
    let forwarded = [
        "-C",
        "packages/app",
        "--profile",
        "review",
        "--",
        "literal $(do-not-execute)",
    ]
    .map(OsString::from);
    let plan = build_plan(&f.project, &forwarded, &f.locations).expect("plan");
    assert_eq!(plan.working_root, nested);
    assert_eq!(enabled(&plan, &skill), Some(true));
    assert_eq!(enabled(&plan, &profile_skill), Some(false));
    assert!(plan.args.ends_with(&forwarded));
    assert_eq!(plan.args.iter().filter(|arg| *arg == "-C").count(), 1);
}

#[test]
fn launcher_does_not_apply_original_project_to_cd_into_another_repository() {
    let f = Fixture::new();
    f.skill(&f.project.join(".agents/skills"), "next", "next");
    f.skill(&f.locations.codex_home.join("skills"), "next", "next");
    let other = f.locations.home.join("other");
    std::fs::create_dir_all(other.join(".git")).expect("other repository");
    let args = [OsString::from("--cd"), other.clone().into_os_string()];
    let plan = build_plan(&f.project, &args, &f.locations).expect("plan");
    assert_eq!(plan.working_root, other);
    assert!(plan.suppressed.is_empty());
    assert!(
        !plan
            .args
            .iter()
            .any(|arg| arg.to_string_lossy().starts_with("skills.config="))
    );
}

#[test]
fn launcher_rejects_invalid_config_instead_of_silently_ignoring_it() {
    let f = Fixture::new();
    f.write(
        &f.project.join(".codex/config.toml"),
        "[[skills.config]]\npath = 5\nenabled = false\n",
    );
    let error = build_plan(&f.project, &[], &f.locations)
        .err()
        .expect("invalid path rejected");
    assert!(error.to_string().contains("skills.config"), "{error:#}");
}

#[cfg(unix)]
#[test]
fn launcher_forwards_non_utf8_and_shell_metacharacters_literally() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    let args = vec![
        OsString::from("--"),
        OsString::from_vec(b"literal\xff$(touch never)".to_vec()),
    ];
    let plan = build_plan(&f.project, &args, &f.locations).expect("plan");
    assert!(plan.args.ends_with(&args));
    assert_eq!(
        &plan.args[..2],
        [OsString::from("-C"), f.project.into_os_string()]
    );
}

#[test]
fn launcher_uses_system_and_user_layers_and_preserves_explicit_cli_override() {
    let f = Fixture::new();
    let system_skill = f.skill(&f.locations.home.join("admin-skills"), "review", "review");
    let shared = f.skill(&f.locations.codex_home.join("skills"), "shared", "shared");
    f.write(
        &f.locations.system_config,
        &format!(
            "[[skills.config]]\npath = {}\nenabled = false\n",
            toml::Value::String(system_skill.to_str().expect("UTF8").to_owned())
        ),
    );
    f.write(
        &f.locations.codex_home.join("config.toml"),
        "[[skills.config]]\npath = 'skills/shared/SKILL.md'\nenabled = false\n",
    );
    let args = ["-c", "skills.config=[]", "--", "--cd=/must/not/parse"].map(OsString::from);
    let plan = build_plan(&f.project, &args, &f.locations).expect("plan");
    assert_eq!(enabled(&plan, &system_skill), Some(false));
    assert_eq!(enabled(&plan, &shared), Some(false));
    assert!(plan.args.ends_with(&args));
    assert_eq!(
        plan.config_layers,
        vec![
            f.locations.system_config,
            f.locations.codex_home.join("config.toml")
        ]
    );
}

#[test]
fn launcher_serializes_non_bmp_and_quoted_paths_as_valid_toml() {
    let f = Fixture::new();
    f.skill(&f.project.join(".agents/skills"), "craft", "craft");
    let path = f.skill(
        &f.locations.codex_home.join("skills"),
        "craft-🚀-'quote'",
        "craft",
    );
    let plan = build_plan(&f.project, &[], &f.locations).expect("plan");
    assert_eq!(enabled(&plan, &path), Some(false));
}

#[cfg(unix)]
#[test]
fn launcher_follows_skill_symlinks_without_looping() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.skill(&f.project.join(".agents/skills"), "next", "next");
    let actual = f.skill(&f.locations.home.join("shared-skills"), "next", "next");
    let personal = f.locations.codex_home.join("skills");
    std::fs::create_dir_all(&personal).expect("personal root");
    symlink(
        actual.parent().expect("skill parent"),
        personal.join("alias"),
    )
    .expect("skill symlink");
    symlink(&personal, personal.join("cycle")).expect("cycle");
    let plan = build_plan(&f.project, &[], &f.locations).expect("plan");
    assert_eq!(plan.suppressed, vec![actual]);
}

#[test]
fn launcher_recognizes_attached_cd_and_profile_flags() {
    let f = Fixture::new();
    let nested = f.project.join("nested");
    std::fs::create_dir(&nested).expect("nested directory");
    for (cd, profile) in [
        ("-Cnested", "-preview"),
        ("-C=nested", "-p=review"),
        ("--cd=nested", "--profile=review"),
    ] {
        let args = [cd, profile].map(OsString::from);
        let invocation = invocation(&f.project, &args).expect("invocation");
        assert_eq!(invocation.working_root, nested);
        assert_eq!(invocation.profile.as_deref(), Some("review"));
    }
}
