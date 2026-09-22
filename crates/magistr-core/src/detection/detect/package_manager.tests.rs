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
fn js_without_evidence_is_unknown() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::JavaScript),
        PackageManager::Unknown
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
fn bun_is_detected_without_a_lockfile_from_independent_evidence() {
    for package in [
        r#"{"packageManager":"bun@1.2.0"}"#,
        r#"{"engines":{"bun":">=1"}}"#,
        r#"{"scripts":{"smoke":"bun run smoke.ts"}}"#,
    ] {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("package.json"), package).unwrap();
        assert_eq!(
            detect_package_manager(tmp.path(), Language::TypeScript),
            PackageManager::Bun
        );
    }
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "bunfig.toml");
    assert_eq!(
        detect_package_manager(tmp.path(), Language::TypeScript),
        PackageManager::Bun
    );
}

#[test]
fn declared_package_manager_takes_precedence() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "bun.lock");
    std::fs::write(
        tmp.path().join("package.json"),
        r#"{"packageManager":"pnpm@10"}"#,
    )
    .unwrap();
    assert_eq!(
        detect_package_manager(tmp.path(), Language::TypeScript),
        PackageManager::Pnpm
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
