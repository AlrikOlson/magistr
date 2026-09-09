use super::*;
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), "").unwrap();
}

fn touch_with(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

#[test]
fn detects_rust() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "Cargo.toml");
    assert_eq!(detect_language(tmp.path()), Language::Rust);
}

#[test]
fn detects_typescript_with_tsconfig() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "package.json");
    touch(tmp.path(), "tsconfig.json");
    assert_eq!(detect_language(tmp.path()), Language::TypeScript);
}

#[test]
fn detects_typescript_from_dep() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"devDependencies":{"typescript":"^5"}}"#,
    );
    assert_eq!(detect_language(tmp.path()), Language::TypeScript);
}

#[test]
fn detects_javascript() {
    let tmp = TempDir::new().unwrap();
    touch_with(tmp.path(), "package.json", r#"{"name":"app"}"#);
    assert_eq!(detect_language(tmp.path()), Language::JavaScript);
}

#[test]
fn detects_python() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "pyproject.toml");
    assert_eq!(detect_language(tmp.path()), Language::Python);
}

#[test]
fn detects_go() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "go.mod");
    assert_eq!(detect_language(tmp.path()), Language::Go);
}

#[test]
fn detects_java_pom() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "pom.xml");
    assert_eq!(detect_language(tmp.path()), Language::Java);
}

#[test]
fn detects_kotlin() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradle.kts");
    assert_eq!(detect_language(tmp.path()), Language::Kotlin);
}

#[test]
fn detects_ruby() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "Gemfile");
    assert_eq!(detect_language(tmp.path()), Language::Ruby);
}

#[test]
fn detects_elixir() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "mix.exs");
    assert_eq!(detect_language(tmp.path()), Language::Elixir);
}

#[test]
fn detects_swift() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "Package.swift");
    assert_eq!(detect_language(tmp.path()), Language::Swift);
}

#[test]
fn detects_cpp() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "CMakeLists.txt");
    assert_eq!(detect_language(tmp.path()), Language::Cpp);
}

#[test]
fn detects_csharp() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "MyApp.csproj");
    assert_eq!(detect_language(tmp.path()), Language::CSharp);
}

#[test]
fn detects_dart() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "pubspec.yaml");
    assert_eq!(detect_language(tmp.path()), Language::Dart);
}

#[test]
fn detects_php() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "composer.json");
    assert_eq!(detect_language(tmp.path()), Language::Php);
}

#[test]
fn detects_scala() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.sbt");
    assert_eq!(detect_language(tmp.path()), Language::Scala);
}

#[test]
fn detects_unknown() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(detect_language(tmp.path()), Language::Unknown);
}

#[test]
fn detect_languages_collects_every_stack_in_a_dual_manifest_dir() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "Cargo.toml");
    touch(tmp.path(), "package.json");
    touch(tmp.path(), "tsconfig.json");
    assert_eq!(
        detect_languages(tmp.path()),
        vec![Language::Rust, Language::TypeScript]
    );
    // The primary is the first entry — detect_language is unchanged by construction.
    assert_eq!(detect_language(tmp.path()), Language::Rust);
}

#[test]
fn detect_languages_single_stack_is_a_singleton() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "Cargo.toml");
    assert_eq!(detect_languages(tmp.path()), vec![Language::Rust]);
}

#[test]
fn detect_languages_empty_dir_is_empty() {
    let tmp = TempDir::new().unwrap();
    assert!(detect_languages(tmp.path()).is_empty());
    assert_eq!(detect_language(tmp.path()), Language::Unknown);
}

#[test]
fn detect_languages_does_not_repeat_a_language() {
    // build.gradle.kts and a kotlin-flavoured build.gradle both say Kotlin.
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "build.gradle.kts");
    touch_with(tmp.path(), "build.gradle", "plugins { kotlin(\"jvm\") }");
    assert_eq!(detect_languages(tmp.path()), vec![Language::Kotlin]);
}
