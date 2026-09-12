use super::*;

#[test]
fn canonical_aliases_share_a_socket_but_distinct_worktrees_do_not() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("root");
    let other = tmp.path().join("other-worktree");
    std::fs::create_dir(&root).expect("root");
    std::fs::create_dir(&other).expect("other");
    let alias = tmp.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).expect("alias");
    assert_eq!(
        workspace_socket_path(&root).expect("root path"),
        workspace_socket_path(&alias).expect("alias path")
    );
    assert_ne!(
        workspace_socket_path(&root).expect("root path"),
        workspace_socket_path(&other).expect("other path")
    );
}

#[test]
fn a_deep_workspace_has_a_short_stable_private_fallback() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("long-root-".repeat(15));
    std::fs::create_dir(&root).expect("deep root");
    let socket = workspace_socket_path(&root).expect("socket");
    assert!(fits_sun_path(&socket), "{}", socket.display());
    assert!(
        socket
            .parent()
            .expect("private directory")
            .file_name()
            .expect("name")
            .to_string_lossy()
            .starts_with("magistr-workspace-")
    );
    assert_eq!(socket, workspace_socket_path(&root).expect("same socket"));
}
