use super::*;
use tempfile::TempDir;

fn write_file(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), content).unwrap();
}

#[test]
fn parses_package_json_deps_and_scripts() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{
                "dependencies": {"react": "^18", "next": "^14"},
                "devDependencies": {"vitest": "^1", "typescript": "^5"},
                "scripts": {"test": "vitest", "build": "next build", "lint": "eslint ."}
            }"#,
    );

    let data = parse_manifests(tmp.path());
    assert!(data.has_dep("react"));
    assert!(data.has_dep("next"));
    assert!(data.has_dep("vitest"));
    assert!(data.has_dep("typescript"));
    assert!(data.has_script("test"));
    assert!(data.has_script("build"));
    assert!(data.has_script("lint"));
    assert!(!data.has_dep("nonexistent"));
}

#[test]
fn parses_cargo_toml_deps_and_features() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Cargo.toml",
        "[package]\nname = \"myapp\"\n\n[dependencies]\naxum = \"0.7\"\nserde = { version = \"1\", features = [\"derive\"] }\n\n[dev-dependencies]\ntempfile = \"3\"\n\n[features]\ndefault = [\"full\"]\nfull = []\n",
    );

    let data = parse_manifests(tmp.path());
    assert!(data.has_dep("axum"));
    assert!(data.has_dep("serde"));
    assert!(data.has_dep("tempfile"));
    assert!(data.cargo_features.contains("default"));
    assert!(data.cargo_features.contains("full"));
}

#[test]
fn parses_pyproject_toml_deps_and_tools() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "pyproject.toml",
        "[project]\nname = \"myapp\"\ndependencies = [\"fastapi>=0.100\", \"uvicorn[standard]\", \"SQLAlchemy>=2.0\"]\n\n[project.optional-dependencies]\ndev = [\"pytest>=7\", \"ruff>=0.1\"]\n\n[tool.ruff]\nline-length = 88\n\n[tool.pytest.ini_options]\ntestpaths = [\"tests\"]\n\n[tool.mypy]\nstrict = true\n",
    );

    let data = parse_manifests(tmp.path());
    assert!(data.has_dep("fastapi"));
    assert!(data.has_dep("uvicorn"));
    assert!(data.has_dep("sqlalchemy"));
    assert!(data.has_dep("pytest"));
    assert!(data.has_dep("ruff"));
    assert!(data.has_python_tool("ruff"));
    assert!(data.has_python_tool("pytest"));
    assert!(data.has_python_tool("mypy"));
}

#[test]
fn parses_go_mod_dependencies() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "go.mod",
        "module example.com/myapp\n\ngo 1.21\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n\tgithub.com/go-chi/chi/v5 v5.0.10\n)\n",
    );

    let data = parse_manifests(tmp.path());
    assert!(data.has_dep("gin"));
    assert!(data.has_dep("github.com/gin-gonic/gin"));
    // chi is nested under v5, last segment is "v5"
    assert!(data.has_dep("github.com/go-chi/chi/v5"));
}

#[test]
fn parses_gemfile_gems() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "Gemfile",
        "source 'https://rubygems.org'\n\ngem 'rails', '~> 7.0'\ngem \"sinatra\"\ngem 'rspec'\n",
    );

    let data = parse_manifests(tmp.path());
    assert!(data.has_dep("rails"));
    assert!(data.has_dep("sinatra"));
    assert!(data.has_dep("rspec"));
}

#[test]
fn parses_mix_exs_deps() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "mix.exs",
        r#"
            defp deps do
              [
                {:phoenix, "~> 1.7"},
                {:phoenix_live_view, "~> 0.20"},
                {:ecto, "~> 3.10"}
              ]
            end
            "#,
    );

    let data = parse_manifests(tmp.path());
    assert!(data.has_dep("phoenix"));
    assert!(data.has_dep("phoenix_live_view"));
    assert!(data.has_dep("ecto"));
}

#[test]
fn handles_missing_files_gracefully() {
    let tmp = TempDir::new().unwrap();
    let data = parse_manifests(tmp.path());
    assert!(data.dependencies.is_empty());
    assert!(data.scripts.is_empty());
}

#[test]
fn handles_malformed_json_gracefully() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "package.json", "{ not valid json");
    let data = parse_manifests(tmp.path());
    assert!(data.dependencies.is_empty());
}

#[test]
fn handles_malformed_toml_gracefully() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "Cargo.toml", "[[[ not valid toml");
    let data = parse_manifests(tmp.path());
    assert!(data.dependencies.is_empty());
}
