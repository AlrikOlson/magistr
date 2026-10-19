use super::*;
use std::fs;

fn policy() -> (tempfile::TempDir, JailPolicy) {
    let root = tempfile::tempdir().expect("temporary root");
    let policy = JailPolicy::new(root.path()).expect("root exists");
    (root, policy)
}

#[test]
fn admits_existing_root_absolute_and_absent_in_root_targets() {
    let (root, policy) = policy();
    fs::write(root.path().join("source.rs"), "fn main() {}\n").unwrap();
    let canonical_root = root.path().canonicalize().unwrap();
    assert_eq!(policy.check_read(".").unwrap(), canonical_root);
    assert_eq!(
        policy.check_read("source.rs").unwrap(),
        canonical_root.join("source.rs")
    );
    assert_eq!(
        policy.check_write("new/deep/file.rs").unwrap(),
        canonical_root.join("new/deep/file.rs")
    );
    assert_eq!(
        policy
            .check_write(canonical_root.join("absolute.rs"))
            .unwrap(),
        canonical_root.join("absolute.rs")
    );
}

#[test]
fn refuses_traversal_absolute_outside_prefix_collision_and_nul() {
    let (root, policy) = policy();
    for candidate in [
        PathBuf::from(".."),
        PathBuf::from("a/../../escape"),
        root.path().with_extension("evil").join("file"),
        PathBuf::from("bad\0file"),
    ] {
        let error = policy
            .check_write(&candidate)
            .expect_err("outside is denied");
        assert_eq!(error.code(), "PATH_OUTSIDE_WORKSPACE");
    }
}

#[test]
fn encoded_traversals_remain_ordinary_names() {
    let (_root, policy) = policy();
    for candidate in [
        "....//escape",
        "%2e%2e/%2e%2e/escape",
        "..%2f..%2fescape",
        "．．/escape",
    ] {
        assert!(
            policy
                .check_write(candidate)
                .unwrap()
                .starts_with(&policy.root)
        );
    }
}

#[test]
fn immutable_protection_is_case_insensitive_and_write_only() {
    let (_root, policy) = policy();
    for target in [
        ".git",
        ".git/hooks/pre-commit",
        ".GIT/config",
        ".magistr/workspace",
        ".MAGISTR/WORKSPACE/keys/workspace.key",
        ".magistr/workspace.toml",
        ".MAGISTR/WORKSPACE.TOML",
    ] {
        assert_eq!(
            policy.check_write(target).unwrap_err().code(),
            "PROTECTED_PATH"
        );
        assert!(policy.check_read(target).is_ok());
    }
    for target in [
        ".github/config",
        ".magistr/workspace-copy/file",
        "nested/.magistr/workspace.toml",
        ".magistr/workspace.toml.backup",
    ] {
        assert!(policy.check_write(target).is_ok());
    }
}

#[test]
fn additionally_loaded_config_paths_cannot_be_written() {
    let (root, policy) = policy();
    let policy = policy
        .with_protected_paths([
            root.path().join("settings/custom.toml"),
            root.path().parent().unwrap().join("org.toml"),
        ])
        .unwrap();
    assert_eq!(
        policy
            .check_write("settings/custom.toml")
            .unwrap_err()
            .code(),
        "PROTECTED_PATH"
    );
    assert!(policy.check_read("settings/custom.toml").is_ok());
    assert!(policy.check_write("settings/other.toml").is_ok());
    assert_eq!(
        policy.check_write("../org.toml").unwrap_err().code(),
        "PATH_OUTSIDE_WORKSPACE"
    );
}

#[test]
fn custom_protection_is_case_insensitive_literal_and_component_bounded() {
    let (_root, policy) = policy();
    let policy = policy
        .with_protected_paths(["private", "literal*.toml"])
        .unwrap();
    for candidate in ["private/file", "PRIVATE/file", "literal*.toml"] {
        assert_eq!(
            policy.check_write(candidate).unwrap_err().code(),
            "PROTECTED_PATH"
        );
    }
    for candidate in ["private-backup/file", "literal-name.toml"] {
        assert!(policy.check_write(candidate).is_ok());
    }
}

#[test]
fn custom_ancestor_protection_covers_the_workspace_subtree() {
    let (_root, policy) = policy();
    let policy = policy.with_protected_paths([".."]).unwrap();
    assert_eq!(
        policy.check_write("source.rs").unwrap_err().code(),
        "PROTECTED_PATH"
    );
    assert!(policy.check_read("source.rs").is_ok());
}

#[test]
fn missing_or_non_directory_root_is_denied() {
    let root = tempfile::tempdir().unwrap();
    assert!(JailPolicy::new(root.path().join("missing")).is_err());
    fs::write(root.path().join("file"), "x").unwrap();
    assert!(JailPolicy::new(root.path().join("file")).is_err());
}

#[cfg(unix)]
#[test]
fn follows_symlinks_before_parent_components_and_denies_dangling_escape() {
    use std::os::unix::fs::symlink;
    let (root, policy) = policy();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("deep/nested")).unwrap();
    symlink(root.path().join("deep/nested"), root.path().join("alias")).unwrap();
    assert_eq!(
        policy.check_write("alias/../new.rs").unwrap(),
        policy.root.join("deep/new.rs")
    );
    symlink(outside.path(), root.path().join("outside")).unwrap();
    symlink(outside.path().join("absent"), root.path().join("dangling")).unwrap();
    for candidate in ["outside/file", "dangling/file", "outside/../escape"] {
        assert_eq!(
            policy.check_write(candidate).unwrap_err().code(),
            "PATH_OUTSIDE_WORKSPACE"
        );
    }
    symlink("loop-b", root.path().join("loop-a")).unwrap();
    symlink("loop-a", root.path().join("loop-b")).unwrap();
    assert!(policy.check_read("loop-a/file").is_err());
}

#[cfg(unix)]
#[test]
fn symlinked_roots_and_protected_aliases_use_canonical_identity() {
    use std::os::unix::fs::symlink;
    let (root, _) = policy();
    fs::create_dir(root.path().join(".git")).unwrap();
    symlink(".git", root.path().join("wormhole")).unwrap();
    let parent = tempfile::tempdir().unwrap();
    symlink(root.path(), parent.path().join("alias")).unwrap();
    let policy = JailPolicy::new(parent.path().join("alias")).unwrap();
    assert_eq!(
        policy.check_write("wormhole/config").unwrap_err().code(),
        "PROTECTED_PATH"
    );
    assert_eq!(
        policy.check_read("file").unwrap(),
        root.path().canonicalize().unwrap().join("file")
    );
}

#[cfg(unix)]
#[test]
fn aliases_of_config_files_are_protected_and_unresolvable_components_fail_closed() {
    use std::os::unix::fs::symlink;
    let (root, policy) = policy();
    fs::write(root.path().join("custom.toml"), "x").unwrap();
    symlink("custom.toml", root.path().join("alias.toml")).unwrap();
    let policy = policy.with_protected_paths(["alias.toml"]).unwrap();
    assert_eq!(
        policy.check_write("custom.toml").unwrap_err().code(),
        "PROTECTED_PATH"
    );
    assert!(policy.check_read("custom.toml/child").is_err());
}

#[cfg(unix)]
#[test]
fn configured_alias_retargeting_preserves_both_original_and_new_targets() {
    use std::os::unix::fs::symlink;
    let (root, policy) = policy();
    fs::write(root.path().join("original.toml"), "x").unwrap();
    fs::write(root.path().join("other.toml"), "x").unwrap();
    symlink("original.toml", root.path().join("alias.toml")).unwrap();
    let policy = policy.with_protected_paths(["alias.toml"]).unwrap();
    fs::remove_file(root.path().join("alias.toml")).unwrap();
    symlink("other.toml", root.path().join("alias.toml")).unwrap();
    for candidate in ["alias.toml", "original.toml", "other.toml"] {
        assert_eq!(
            policy.check_write(candidate).unwrap_err().code(),
            "PROTECTED_PATH"
        );
    }
}

#[cfg(unix)]
#[test]
fn protected_directory_alias_cannot_make_its_target_writable() {
    use std::os::unix::fs::symlink;
    let (root, policy) = policy();
    fs::create_dir(root.path().join("metadata")).unwrap();
    symlink("metadata", root.path().join(".git")).unwrap();
    for candidate in [".git/config", "metadata/config"] {
        assert_eq!(
            policy.check_write(candidate).unwrap_err().code(),
            "PROTECTED_PATH"
        );
    }
}
