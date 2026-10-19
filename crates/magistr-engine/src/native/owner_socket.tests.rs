use super::*;

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_runtime_dir(tmp.path().join("run"))
}

#[test]
fn the_socket_sits_beside_the_lock_when_the_path_is_short() {
    let cfg = NativeConfig::from_lookup("/p", |_| None).with_data_dir("/d");
    let host = owner_socket_path(&cfg, "lease", WriterRole::Host);
    assert_eq!(
        host,
        PathBuf::from("/d/roadmap/sessions/lease.project-owner.lock.sock")
    );
    let service = owner_socket_path(&cfg, "lease", WriterRole::Service);
    assert_eq!(
        service,
        PathBuf::from("/d/roadmap/sessions/lease.project-owner.lock.sock")
    );
    // A stateless store keys the endpoint under the project root.
    let stateless = cfg.persist(false);
    assert_eq!(
        owner_socket_path(&stateless, "lease", WriterRole::Host),
        PathBuf::from("/p/.magistr/project-owner.lock.sock")
    );
}

#[test]
fn a_deep_store_falls_back_to_the_runtime_directory_and_stays_stable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let deep = tmp.path().join("x".repeat(120));
    let cfg = config(&tmp).with_data_dir(&deep);
    let first = owner_socket_path(&cfg, "lease", WriterRole::Host);
    assert_eq!(first.parent(), Some(tmp.path().join("run").as_path()));
    let name = first.file_name().expect("name").to_string_lossy();
    assert!(
        name.starts_with("magistr-") && name.ends_with(".workflow.sock"),
        "{name}"
    );
    assert_eq!(first, owner_socket_path(&cfg, "lease", WriterRole::Host));
    // Another project on the same deep store gets its own rendezvous.
    assert_ne!(first, owner_socket_path(&cfg, "other", WriterRole::Host));
    assert_eq!(first, owner_socket_path(&cfg, "lease", WriterRole::Service));
}

#[cfg(unix)]
#[test]
fn store_aliases_share_one_rendezvous_before_and_after_creation() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let alias = tmp.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let deep = "d".repeat(120);
    let a = config(&tmp).with_data_dir(real.join(&deep));
    let b = config(&tmp).with_data_dir(alias.join(".").join(&deep));
    let before = owner_socket_path(&a, "lease", WriterRole::Host);
    assert_eq!(before, owner_socket_path(&b, "lease", WriterRole::Service));
    std::fs::create_dir_all(real.join(&deep).join("roadmap/sessions")).unwrap();
    assert_eq!(before, owner_socket_path(&b, "lease", WriterRole::Service));
}
