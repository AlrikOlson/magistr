//! Runtime transport may depend on semantic values and Tokio, never a surface or host.

use std::path::Path;

fn forbidden_dependencies(source: &str) -> Vec<String> {
    fn visit(value: &toml::Value, errors: &mut Vec<String>) {
        let Some(table) = value.as_table() else {
            return;
        };
        for (key, value) in table {
            if matches!(
                key.as_str(),
                "dependencies" | "build-dependencies" | "dev-dependencies"
            ) {
                for (alias, spec) in value.as_table().expect("dependency table") {
                    let name = spec
                        .get("package")
                        .and_then(toml::Value::as_str)
                        .unwrap_or(alias);
                    let allowed =
                        matches!(name, "magistr-cockpit" | "magistr-application" | "tokio")
                            || (key == "dev-dependencies"
                                && matches!(name, "magistr-domain" | "toml"));
                    if !allowed {
                        errors.push(name.to_owned());
                    }
                }
            } else {
                visit(value, errors);
            }
        }
    }
    let manifest: toml::Value = toml::from_str(source).expect("valid Cargo manifest");
    let mut errors = Vec::new();
    visit(&manifest, &mut errors);
    errors
}

#[test]
fn transport_has_no_surface_provider_or_concrete_service_dependency() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("Cargo.toml")).expect("runtime manifest");
    assert_eq!(forbidden_dependencies(&source), Vec::<String>::new());
}

#[test]
fn optional_renamed_and_platform_dependencies_cannot_hide_a_host() {
    for source in [
        "[dependencies]\nmagistr-tui = '1'\n",
        "[dependencies]\ntransport = { package = 'magistr-acp', version = '1', optional = true }\n",
        "[target.'cfg(unix)'.dependencies]\nstore = { package = 'magistr-engine', version = '1' }\n",
        "[build-dependencies]\nmagistr-cli = '1'\n",
        "[dev-dependencies]\nmagistr-tui = '1'\n",
    ] {
        assert_eq!(forbidden_dependencies(source).len(), 1, "{source}");
    }
}
