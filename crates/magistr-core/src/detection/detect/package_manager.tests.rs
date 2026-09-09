use super::*;
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "").unwrap();
}

#[test]
fn rust_uses_cargo() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Rust),
        PackageManager::Cargo
    );
}

#[test]
fn js_defaults_to_npm() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::JavaScript),
        PackageManager::Npm
    );
}

#[test]
fn js_detects_pnpm() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "pnpm-lock.yaml");
    assert_eq!(
        detect_package_manager(tmp.path(), Language::TypeScript),
        PackageManager::Pnpm
    );
}

#[test]
fn js_detects_yarn() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "yarn.lock");
    assert_eq!(
        detect_package_manager(tmp.path(), Language::JavaScript),
        PackageManager::Yarn
    );
}

#[test]
fn js_detects_bun() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "bun.lockb");
    assert_eq!(
        detect_package_manager(tmp.path(), Language::TypeScript),
        PackageManager::Bun
    );
}

#[test]
fn python_detects_uv() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "uv.lock");
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Python),
        PackageManager::Uv
    );
}

#[test]
fn python_detects_poetry() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "poetry.lock");
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Python),
        PackageManager::Poetry
    );
}

#[test]
fn go_uses_go() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Go),
        PackageManager::Go
    );
}

#[test]
fn dart_uses_pub() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Dart),
        PackageManager::Pub
    );
}

#[test]
fn php_uses_composer() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Php),
        PackageManager::Composer
    );
}

#[test]
fn scala_uses_sbt() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::Scala),
        PackageManager::Sbt
    );
}
