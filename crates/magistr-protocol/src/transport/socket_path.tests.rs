use super::*;

#[test]
fn missing_home_reports_a_not_found_error() {
    let err = home_dir_from(Err(std::env::VarError::NotPresent)).expect_err("missing home");
    let TransportError::Io(err) = err else {
        panic!("expected an IO error");
    };
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(err.to_string(), "HOME environment variable not set");
}

#[test]
fn server_socket_path_appends_sock_extension() {
    let home = home_dir_from(Ok("/home/testuser".to_owned())).expect("home supplied");
    let path = server_socket_path_under(&home, "engine");
    assert_eq!(
        path,
        PathBuf::from("/home/testuser/.magistr/sockets/engine.sock")
    );
}

#[test]
fn socket_dir_under_computes_correct_path() {
    let base = PathBuf::from("/home/testuser");
    let dir = socket_dir_under(&base);
    assert_eq!(dir, PathBuf::from("/home/testuser/.magistr/sockets"));
}

#[test]
fn ensure_socket_dir_under_creates_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let dir = ensure_socket_dir_under(tmp.path()).expect("should create");
    assert!(dir.exists());
    assert!(dir.ends_with(".magistr/sockets"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&dir).expect("metadata").permissions();
        assert_eq!(perms.mode() & 0o777, 0o700);
    }
}

#[test]
fn ensure_socket_dir_under_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let dir1 = ensure_socket_dir_under(tmp.path()).expect("first call");
    let dir2 = ensure_socket_dir_under(tmp.path()).expect("second call");
    assert_eq!(dir1, dir2);
    assert!(dir1.exists());
}
