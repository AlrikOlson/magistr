use std::path::Path;
use tempfile::TempDir;

#[path = "support/magistr_command.rs"]
mod magistr_command;

fn write_file(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), content).unwrap();
}

/// Run magistr detect on a fixture directory and return parsed JSON output.
fn detect_json(dir: &Path) -> serde_json::Value {
    let output = magistr_command::magistr_command()
        .args(["detect", &dir.display().to_string()])
        .output()
        .expect("failed to run magistr detect");

    assert!(
        output.status.success(),
        "magistr detect failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("invalid JSON from magistr detect")
}

#[test]
fn rust_axum_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"my-api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\naxum = \"0.7\"\ntokio = { version = \"1\", features = [\"full\"] }\nserde = { version = \"1\", features = [\"derive\"] }\n",
    );
    write_file(tmp.path(), "src/main.rs", "fn main() {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "rust");
    assert_eq!(json["framework"], "axum");
    assert_eq!(json["package_manager"], "cargo");
    assert_eq!(json["test_framework"], "cargo-test");
    assert_eq!(json["linter"], "clippy");
    assert_eq!(json["formatter"], "rustfmt");
    assert_eq!(json["project_name"], "my-api");
}

#[test]
fn typescript_nextjs_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"my-app","dependencies":{"next":"14","react":"18","react-dom":"18"},"devDependencies":{"typescript":"5","vitest":"^1"}}"#,
    );
    write_file(tmp.path(), "tsconfig.json", "{}");
    write_file(tmp.path(), "next.config.js", "module.exports = {}");
    write_file(tmp.path(), "vitest.config.ts", "export default {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["framework"], "next");
    assert_eq!(json["package_manager"], "unknown");
    assert_eq!(json["test_framework"], "vitest");
    assert_eq!(json["linter"], "none");
    assert_eq!(json["formatter"], "none");
    assert_eq!(json["project_name"], "my-app");
}

#[test]
fn typescript_biome_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"biome-app","dependencies":{"react":"18"},"devDependencies":{"@biomejs/biome":"^1","typescript":"5"}}"#,
    );
    write_file(tmp.path(), "tsconfig.json", "{}");
    write_file(tmp.path(), "biome.json", "{}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["linter"], "biome");
    assert_eq!(json["formatter"], "biome");
}

#[test]
fn python_django_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"mysite\"\ndependencies = [\"django>=4.2\"]\n\n[tool.ruff]\nline-length = 88\n",
    );
    write_file(tmp.path(), "manage.py", "#!/usr/bin/env python");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "python");
    assert_eq!(json["framework"], "django");
    assert_eq!(json["linter"], "ruff");
    assert_eq!(json["formatter"], "ruff");
    assert_eq!(json["project_name"], "mysite");
}

#[test]
fn python_fastapi_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"my-api\"\ndependencies = [\"fastapi>=0.100\", \"uvicorn\"]\n\n[project.optional-dependencies]\ndev = [\"pytest>=7\", \"ruff>=0.1\"]\n\n[tool.ruff]\nline-length = 88\n\n[tool.pytest.ini_options]\ntestpaths = [\"tests\"]\n",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "python");
    assert_eq!(json["framework"], "fastapi");
    assert_eq!(json["linter"], "ruff");
    assert_eq!(json["formatter"], "ruff");
}

#[test]
fn go_chi_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.mod",
        "module example.com/myapp\n\ngo 1.21\n\nrequire (\n\tgithub.com/go-chi/chi/v5 v5.0.10\n)\n",
    );
    write_file(tmp.path(), "main.go", "package main");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "go");
    assert_eq!(json["framework"], "chi");
    assert_eq!(json["package_manager"], "go");
    assert_eq!(json["test_framework"], "go-test");
}

#[test]
fn go_gin_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.mod",
        "module example.com/api\n\ngo 1.21\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n)\n",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "go");
    assert_eq!(json["framework"], "gin");
}

#[test]
fn java_spring_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pom.xml",
        "<project><parent><artifactId>spring-boot-starter-parent</artifactId></parent></project>",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "java");
    assert_eq!(json["framework"], "spring");
    assert_eq!(json["package_manager"], "maven");
}

#[test]
fn ruby_rails_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Gemfile",
        "source 'https://rubygems.org'\n\ngem 'rails', '~> 7.0'\ngem 'rspec-rails'\n",
    );
    write_file(
        tmp.path(),
        "config/routes.rb",
        "Rails.application.routes.draw do\nend\n",
    );
    write_file(tmp.path(), ".rspec", "--format documentation");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "ruby");
    assert_eq!(json["framework"], "rails");
    assert_eq!(json["package_manager"], "bundler");
    assert_eq!(json["test_framework"], "rspec");
}

#[test]
fn bare_javascript_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"simple-app","scripts":{"start":"node index.js"}}"#,
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "javascript");
    assert_eq!(json["framework"], "none");
    assert_eq!(json["package_manager"], "unknown");
}

#[test]
fn bare_python_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"mylib\"\nversion = \"0.1.0\"\n",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "python");
    assert_eq!(json["framework"], "none");
}

#[test]
fn rust_workspace_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n\n[package]\nname = \"workspace-root\"\nversion = \"0.1.0\"\n",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "rust");
    assert_eq!(json["workspace"], "cargo");
}

#[test]
fn npm_monorepo_with_turborepo() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"monorepo","workspaces":["apps/*","packages/*"],"devDependencies":{"typescript":"5"}}"#,
    );
    write_file(tmp.path(), "turbo.json", "{}");
    write_file(tmp.path(), "tsconfig.json", "{}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["workspace"], "turborepo");
}

#[test]
fn elixir_phoenix_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "mix.exs",
        "defmodule MyApp.MixProject do\n  defp deps do\n    [{:phoenix, \"~> 1.7\"}, {:ecto, \"~> 3.10\"}]\n  end\nend\n",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "elixir");
    assert_eq!(json["framework"], "phoenix");
    assert_eq!(json["package_manager"], "mix");
}

#[test]
fn kotlin_ktor_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "build.gradle.kts",
        "plugins {\n    kotlin(\"jvm\")\n}\n\ndependencies {\n    implementation(\"io.ktor:ktor-server-core:2.3.0\")\n}\n",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "kotlin");
    assert_eq!(json["framework"], "ktor");
}

#[test]
fn csharp_aspnet_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "MyApp.csproj",
        "<Project Sdk=\"Microsoft.NET.Sdk.Web\">\n<PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup>\n</Project>",
    );

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "csharp");
    assert_eq!(json["framework"], "aspnet");
}

#[test]
fn python_flask_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"flask-app\"\ndependencies = [\"flask>=3.0\"]\n",
    );
    write_file(tmp.path(), "app.py", "from flask import Flask");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "python");
    assert_eq!(json["framework"], "flask");
}

#[test]
fn typescript_express_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"express-api","dependencies":{"express":"^4.18"},"devDependencies":{"typescript":"5","jest":"^29"}}"#,
    );
    write_file(tmp.path(), "tsconfig.json", "{}");
    write_file(tmp.path(), "jest.config.ts", "export default {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["framework"], "express");
    assert_eq!(json["test_framework"], "jest");
}

#[test]
fn typescript_vite_react_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"vite-react","dependencies":{"react":"18","react-dom":"18"},"devDependencies":{"vite":"^5","typescript":"5","vitest":"^1","@vitejs/plugin-react":"^4"}}"#,
    );
    write_file(tmp.path(), "tsconfig.json", "{}");
    write_file(tmp.path(), "vite.config.ts", "export default {}");
    write_file(tmp.path(), "vitest.config.ts", "export default {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["framework"], "vite");
    assert_eq!(json["test_framework"], "vitest");
}

#[test]
fn typescript_sveltekit_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"sk-app","devDependencies":{"@sveltejs/kit":"^2","svelte":"^4","typescript":"5"}}"#,
    );
    write_file(tmp.path(), "tsconfig.json", "{}");
    write_file(tmp.path(), "svelte.config.js", "export default {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["framework"], "sveltekit");
}

#[test]
fn pnpm_workspace_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"pnpm-mono","devDependencies":{"typescript":"5"}}"#,
    );
    write_file(
        tmp.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - 'apps/*'\n  - 'packages/*'\n",
    );
    write_file(tmp.path(), "pnpm-lock.yaml", "lockfileVersion: 9");
    write_file(tmp.path(), "tsconfig.json", "{}");

    let json = detect_json(tmp.path());
    assert_eq!(json["package_manager"], "pnpm");
    assert_eq!(json["workspace"], "pnpm");
}

#[test]
fn go_echo_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.mod",
        "module example.com/echoapp\n\ngo 1.21\n\nrequire (\n\tgithub.com/labstack/echo/v4 v4.11.3\n)\n",
    );
    write_file(tmp.path(), "main.go", "package main");
    write_file(tmp.path(), ".golangci.yml", "run:\n  timeout: 5m\n");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "go");
    assert_eq!(json["framework"], "echo");
    assert_eq!(json["linter"], "golangci-lint");
}

#[test]
fn rust_tauri_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"my-desktop-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntauri = { version = \"2\", features = [] }\nserde = { version = \"1\", features = [\"derive\"] }\n",
    );
    write_file(tmp.path(), "src/main.rs", "fn main() {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "rust");
    assert_eq!(json["framework"], "tauri");
    assert_eq!(json["package_manager"], "cargo");
}

#[test]
fn typescript_nuxt_project() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"nuxt-app","devDependencies":{"nuxt":"^3","typescript":"5"}}"#,
    );
    write_file(tmp.path(), "tsconfig.json", "{}");
    write_file(tmp.path(), "nuxt.config.ts", "export default {}");

    let json = detect_json(tmp.path());
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["framework"], "nuxt");
}

#[test]
fn smart_detect_rust_high_confidence() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"conf-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    write_file(tmp.path(), "src/main.rs", "fn main() {}");

    let output = magistr_command::magistr_command()
        .args([
            "detect",
            "--smart",
            "--json",
            &tmp.path().display().to_string(),
        ])
        .output()
        .expect("failed to run magistr detect --smart");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("invalid JSON");

    // Smart detect wraps values with confidence
    let lang = &json["data"]["language"];
    assert_eq!(lang["value"], "rust");
    assert!(lang["confidence"].as_u64().unwrap() >= 90);
}

#[test]
fn smart_detect_low_confidence_flagged() {
    let tmp = TempDir::new().unwrap();
    // A minimal Python project with no tooling config — should have low confidence on linter/formatter
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"bare-py\"\nversion = \"0.1.0\"\n",
    );

    let output = magistr_command::magistr_command()
        .args([
            "detect",
            "--smart",
            "--json",
            &tmp.path().display().to_string(),
        ])
        .output()
        .expect("failed to run magistr detect --smart");

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("invalid JSON");

    let data = &json["data"];
    // Language should be high confidence
    assert!(data["language"]["confidence"].as_u64().unwrap() >= 80);
    // Test framework should be low confidence (defaulted)
    assert!(data["test_framework"]["confidence"].as_u64().unwrap() < 60);
}
