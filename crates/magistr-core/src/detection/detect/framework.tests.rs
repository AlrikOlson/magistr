use super::*;
use crate::detection::detect::config_parser::parse_manifests;
use tempfile::TempDir;

fn touch(dir: &Path, name: &str) {
    if let Some(parent) = dir.join(name).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(dir.join(name), "").unwrap();
}

fn touch_with(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

/// Helper: detect framework with auto-parsed manifests
fn detect(dir: &Path, lang: Language) -> Framework {
    let manifest = parse_manifests(dir);
    detect_framework(dir, lang, &manifest)
}

#[test]
fn detects_next_from_config() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "next.config.js");
    assert_eq!(detect(tmp.path(), Language::TypeScript), Framework::Next);
}

#[test]
fn detects_next_from_package_json() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"dependencies":{"next":"14"}}"#,
    );
    assert_eq!(detect(tmp.path(), Language::TypeScript), Framework::Next);
}

#[test]
fn detects_vite() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "vite.config.ts");
    assert_eq!(detect(tmp.path(), Language::TypeScript), Framework::Vite);
}

#[test]
fn detects_django() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "manage.py");
    assert_eq!(detect(tmp.path(), Language::Python), Framework::Django);
}

#[test]
fn detects_fastapi() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pyproject.toml",
        "[project]\ndependencies = [\"fastapi>=0.100\"]\n",
    );
    assert_eq!(detect(tmp.path(), Language::Python), Framework::FastApi);
}

#[test]
fn detects_rails() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "config/routes.rb");
    assert_eq!(detect(tmp.path(), Language::Ruby), Framework::Rails);
}

#[test]
fn detects_gin() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "go.mod",
        "module example.com/app\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n)\n",
    );
    assert_eq!(detect(tmp.path(), Language::Go), Framework::Gin);
}

#[test]
fn detects_axum() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\naxum = \"0.7\"\ntokio = \"1\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Axum);
}

#[test]
fn detects_actix() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\nactix-web = \"4\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Actix);
}

#[test]
fn detects_rocket() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\nrocket = \"0.5\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Rocket);
}

#[test]
fn detects_tauri_from_config() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src-tauri/tauri.conf.json");
    touch_with(tmp.path(), "Cargo.toml", "[package]\nname = \"app\"\n");
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Tauri);
}

#[test]
fn detects_tauri_from_dep() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ntauri = \"2\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Tauri);
}

#[test]
fn detects_leptos() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\nleptos = \"0.6\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Leptos);
}

#[test]
fn detects_bevy() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"game\"\n\n[dependencies]\nbevy = \"0.13\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Bevy);
}

#[test]
fn detects_dioxus() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ndioxus = \"0.5\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::Dioxus);
}

#[test]
fn rust_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"lib\"\n\n[dependencies]\nserde = \"1\"\n",
    );
    assert_eq!(detect(tmp.path(), Language::Rust), Framework::None);
}

#[test]
fn detects_chi() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "go.mod",
        "module example.com/app\n\nrequire (\n\tgithub.com/go-chi/chi/v5 v5.0.0\n)\n",
    );
    assert_eq!(detect(tmp.path(), Language::Go), Framework::Chi);
}

#[test]
fn no_framework_for_bare_js() {
    let tmp = TempDir::new().unwrap();
    touch_with(tmp.path(), "package.json", r#"{"name":"app"}"#);
    assert_eq!(detect(tmp.path(), Language::JavaScript), Framework::None);
}

// Java framework tests
#[test]
fn detects_spring_from_pom() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pom.xml",
        "<parent><artifactId>spring-boot-starter-parent</artifactId></parent>",
    );
    assert_eq!(detect(tmp.path(), Language::Java), Framework::Spring);
}

#[test]
fn detects_spring_from_gradle() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "build.gradle",
        "plugins { id 'org.springframework.boot' version '3.2.0' }",
    );
    assert_eq!(detect(tmp.path(), Language::Java), Framework::Spring);
}

#[test]
fn detects_spring_from_config_files() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "pom.xml");
    touch(tmp.path(), "src/main/resources/application.properties");
    assert_eq!(detect(tmp.path(), Language::Java), Framework::Spring);
}

#[test]
fn detects_quarkus_from_pom() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pom.xml",
        "<dependency><groupId>io.quarkus</groupId></dependency>",
    );
    assert_eq!(detect(tmp.path(), Language::Java), Framework::Quarkus);
}

#[test]
fn java_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pom.xml",
        "<project><modelVersion>4.0.0</modelVersion></project>",
    );
    assert_eq!(detect(tmp.path(), Language::Java), Framework::None);
}

// Kotlin framework tests
#[test]
fn detects_ktor_from_gradle_kts() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "build.gradle.kts",
        "dependencies {\n    implementation(\"io.ktor:ktor-server-core:2.3.0\")\n}",
    );
    assert_eq!(detect(tmp.path(), Language::Kotlin), Framework::Ktor);
}

#[test]
fn detects_spring_for_kotlin() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "build.gradle.kts",
        "plugins {\n    id(\"org.springframework.boot\") version \"3.2.0\"\n}",
    );
    assert_eq!(detect(tmp.path(), Language::Kotlin), Framework::Spring);
}

#[test]
fn kotlin_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "build.gradle.kts",
        "plugins {\n    kotlin(\"jvm\") version \"1.9.0\"\n}",
    );
    assert_eq!(detect(tmp.path(), Language::Kotlin), Framework::None);
}

// Swift framework tests
#[test]
fn detects_vapor() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Package.swift",
        "dependencies: [.package(url: \"https://github.com/vapor/vapor.git\", from: \"4.0.0\")]",
    );
    assert_eq!(detect(tmp.path(), Language::Swift), Framework::Vapor);
}

#[test]
fn swift_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "Package.swift",
        "let package = Package(name: \"MyLib\")",
    );
    assert_eq!(detect(tmp.path(), Language::Swift), Framework::None);
}

// C# framework tests
#[test]
fn detects_aspnet_from_csproj() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "MyApp.csproj",
        "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>",
    );
    assert_eq!(detect(tmp.path(), Language::CSharp), Framework::AspNet);
}

#[test]
fn detects_aspnet_from_convention() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "Program.cs");
    touch(tmp.path(), "appsettings.json");
    assert_eq!(detect(tmp.path(), Language::CSharp), Framework::AspNet);
}

#[test]
fn csharp_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "MyLib.csproj",
        "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>",
    );
    assert_eq!(detect(tmp.path(), Language::CSharp), Framework::None);
}

// --- New framework detection tests ---

#[test]
fn detects_flutter_from_pubspec() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pubspec.yaml",
        "dependencies:\n  flutter:\n    sdk: flutter\n",
    );
    assert_eq!(detect(tmp.path(), Language::Dart), Framework::Flutter);
}

#[test]
fn dart_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pubspec.yaml",
        "name: my_dart_app\ndependencies:\n  http: ^1.0.0\n",
    );
    assert_eq!(detect(tmp.path(), Language::Dart), Framework::None);
}

#[test]
fn detects_nestjs_from_package_json() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"dependencies":{"@nestjs/core":"^10"}}"#,
    );
    assert_eq!(detect(tmp.path(), Language::TypeScript), Framework::NestJS);
}

#[test]
fn detects_react_native_from_package_json() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "package.json",
        r#"{"dependencies":{"react-native":"0.73"}}"#,
    );
    assert_eq!(
        detect(tmp.path(), Language::TypeScript),
        Framework::ReactNative
    );
}

#[test]
fn detects_micronaut_from_pom() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "pom.xml",
        "<dependency><groupId>io.micronaut</groupId></dependency>",
    );
    assert_eq!(detect(tmp.path(), Language::Java), Framework::Micronaut);
}

#[test]
fn detects_laravel_from_artisan() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "artisan");
    touch(tmp.path(), "composer.json");
    assert_eq!(detect(tmp.path(), Language::Php), Framework::Laravel);
}

#[test]
fn detects_laravel_from_composer() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "composer.json",
        r#"{"require":{"laravel/framework":"^11"}}"#,
    );
    assert_eq!(detect(tmp.path(), Language::Php), Framework::Laravel);
}

#[test]
fn php_no_framework_for_plain_project() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "composer.json",
        r#"{"require":{"monolog/monolog":"^3"}}"#,
    );
    assert_eq!(detect(tmp.path(), Language::Php), Framework::None);
}

#[test]
fn detects_blazor_from_csproj() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "MyApp.csproj",
        "<PackageReference Include=\"Microsoft.AspNetCore.Components.Blazor\" />",
    );
    assert_eq!(detect(tmp.path(), Language::CSharp), Framework::Blazor);
}

#[test]
fn detects_maui_from_csproj() {
    let tmp = TempDir::new().unwrap();
    touch_with(
        tmp.path(),
        "MyApp.csproj",
        "<Project><UseMaui>true</UseMaui><PackageReference Include=\"Microsoft.Maui\" /></Project>",
    );
    assert_eq!(detect(tmp.path(), Language::CSharp), Framework::Maui);
}
