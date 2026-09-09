use super::*;
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), "").unwrap();
}

fn touch_with(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), content).unwrap();
}

#[test]
fn smart_rust_project_high_confidence() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n",
    );

    let result = detect_stack_smart(tmp.path()).unwrap();
    assert_eq!(result.language.value, Language::Rust);
    assert!(result.language.confidence.is_high());
    assert_eq!(result.package_manager.value, PackageManager::Cargo);
    assert!(result.package_manager.confidence.is_high());
    assert!(
        result.low_confidence_categories().is_empty()
            || !result.low_confidence_categories().contains(&"language")
    );
}

#[test]
fn smart_ts_next_app_router() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"name":"my-app","dependencies":{"next":"14","react":"18"},"devDependencies":{"typescript":"5"}}"#,
    );
    touch(tmp.path(), "tsconfig.json");
    touch(tmp.path(), "next.config.js");
    std::fs::create_dir_all(tmp.path().join("app")).unwrap();

    let result = detect_stack_smart(tmp.path()).unwrap();
    assert_eq!(result.language.value, Language::TypeScript);
    assert!(result.language.confidence.is_high());
    assert_eq!(result.framework.value, Framework::Next);
    assert!(result.framework.confidence.is_high());
    assert_eq!(result.framework_detail.as_deref(), Some("App Router"));
}

#[test]
fn smart_ts_next_pages_router() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"name":"my-app","dependencies":{"next":"14"},"devDependencies":{"typescript":"5"}}"#,
    );
    touch(tmp.path(), "tsconfig.json");
    touch(tmp.path(), "next.config.js");
    std::fs::create_dir_all(tmp.path().join("pages")).unwrap();

    let result = detect_stack_smart(tmp.path()).unwrap();
    assert_eq!(result.framework_detail.as_deref(), Some("Pages Router"));
}

#[test]
fn smart_ts_next_hybrid_router() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"name":"my-app","dependencies":{"next":"14"},"devDependencies":{"typescript":"5"}}"#,
    );
    touch(tmp.path(), "tsconfig.json");
    touch(tmp.path(), "next.config.js");
    std::fs::create_dir_all(tmp.path().join("app")).unwrap();
    std::fs::create_dir_all(tmp.path().join("pages")).unwrap();

    let result = detect_stack_smart(tmp.path()).unwrap();
    assert_eq!(
        result.framework_detail.as_deref(),
        Some("Hybrid (App Router + Pages)")
    );
}

#[test]
fn smart_django_rest_framework() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"my-api\"\ndependencies = [\"django\", \"djangorestframework\"]\n",
    );
    touch(tmp.path(), "manage.py");

    let result = detect_stack_smart(tmp.path()).unwrap();
    assert_eq!(result.framework.value, Framework::Django);
    assert!(result.framework.confidence.is_high());
    assert_eq!(result.framework_detail.as_deref(), Some("REST Framework"));
}

#[test]
fn smart_detection_flags_low_confidence() {
    let tmp = TempDir::new().unwrap();
    // Empty directory → Unknown language → low/none confidence
    let result = detect_stack_smart(tmp.path()).unwrap();
    assert_eq!(result.language.value, Language::Unknown);
    assert!(result.language.needs_override());
    assert!(result.low_confidence_categories().contains(&"language"));
}

#[test]
fn smart_to_stack_preserves_values() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    );

    let smart = detect_stack_smart(tmp.path()).unwrap();
    let stack = smart.to_stack();
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.package_manager, PackageManager::Cargo);
    assert_eq!(stack.project_name, smart.project_name);
}

#[test]
fn next_detail_none_without_dirs() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(detect_next_detail(tmp.path()), None);
}

#[test]
fn django_detail_none_without_rest() {
    let manifest = ManifestData::default();
    assert_eq!(detect_django_detail(&manifest), None);
}

#[test]
fn errors_on_nonexistent_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = detect_stack_smart(&tmp.path().join("nonexistent"));
    assert!(result.is_err());
}
