use super::*;

#[test]
fn configuration_is_shared_but_other_setup_files_stay_in_the_copy() {
    let temp = tempfile::tempdir().unwrap();
    let main = temp.path().join("main");
    let linked = temp.path().join("task");
    let metadata = main.join(".git/worktrees/task");
    std::fs::create_dir_all(&metadata).unwrap();
    std::fs::create_dir_all(&linked).unwrap();
    std::fs::write(
        linked.join(".git"),
        format!("gitdir: {}", metadata.display()),
    )
    .unwrap();
    std::fs::write(metadata.join("commondir"), "../..").unwrap();
    std::fs::write(
        metadata.join("gitdir"),
        linked.join(".git").display().to_string(),
    )
    .unwrap();
    assert_eq!(
        target_root(&linked, ".magistr/project.json").unwrap(),
        main.canonicalize().unwrap()
    );
    assert_eq!(target_root(&linked, ".mcp.json").unwrap(), linked);
    std::fs::write(
        metadata.join("gitdir"),
        main.join(".git").display().to_string(),
    )
    .unwrap();
    assert!(target_root(&linked, ".magistr/project.json").is_err());
}
