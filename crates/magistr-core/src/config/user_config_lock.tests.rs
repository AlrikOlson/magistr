use super::*;
#[test]
fn release_does_not_wait_for_an_inherited_descriptor() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.lock");
    let first = UserConfigLock::acquire(&path, &path).unwrap();
    let inherited = first.0.try_clone().unwrap();
    drop(first);
    let _next = UserConfigLock::acquire(&path, &path).unwrap();
    drop(inherited);
}
