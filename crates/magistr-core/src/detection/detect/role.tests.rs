use super::*;

#[test]
fn classify_frontend_by_framework() {
    let role = classify_role(
        Path::new("."),
        Language::TypeScript,
        Some(Framework::React),
        &[StackMarker::PackageJson],
        &[],
        false,
    );
    assert_eq!(role, ProjectRole::Frontend);
}

#[test]
fn classify_backend_by_framework() {
    let role = classify_role(
        Path::new("."),
        Language::TypeScript,
        Some(Framework::Express),
        &[StackMarker::PackageJson],
        &[],
        false,
    );
    assert_eq!(role, ProjectRole::Backend);
}

#[test]
fn classify_fullstack_next() {
    let role = classify_role(
        Path::new("."),
        Language::TypeScript,
        Some(Framework::Next),
        &[StackMarker::PackageJson],
        &[],
        false,
    );
    assert_eq!(role, ProjectRole::Fullstack);
}

#[test]
fn classify_cli_by_dep() {
    let role = classify_role(
        Path::new("."),
        Language::Rust,
        Some(Framework::None),
        &[StackMarker::CargoToml],
        &["clap".to_string(), "serde".to_string()],
        false,
    );
    assert_eq!(role, ProjectRole::Cli);
}

#[test]
fn classify_service_docker_compose() {
    let role = classify_role(
        Path::new("."),
        Language::Go,
        Some(Framework::None),
        &[StackMarker::GoMod],
        &[],
        true,
    );
    assert_eq!(role, ProjectRole::Service);
}

#[test]
fn classify_frontend_docker_compose_with_react() {
    let role = classify_role(
        Path::new("."),
        Language::TypeScript,
        Some(Framework::React),
        &[StackMarker::PackageJson],
        &[],
        true,
    );
    assert_eq!(role, ProjectRole::Frontend);
}

#[test]
fn classify_backend_docker_compose_with_express() {
    let role = classify_role(
        Path::new("."),
        Language::TypeScript,
        Some(Framework::Express),
        &[StackMarker::PackageJson],
        &[],
        true,
    );
    assert_eq!(role, ProjectRole::Service);
}

#[test]
fn classify_unknown_no_signals() {
    let role = classify_role(Path::new("."), Language::Unknown, None, &[], &[], false);
    assert_eq!(role, ProjectRole::Unknown);
}

#[test]
fn classify_desktop_app_tauri() {
    let role = classify_role(
        Path::new("."),
        Language::Rust,
        Some(Framework::Tauri),
        &[StackMarker::CargoToml],
        &[],
        false,
    );
    assert_eq!(role, ProjectRole::Frontend);
}

#[test]
fn classify_fullstack_rust_leptos() {
    let role = classify_role(
        Path::new("."),
        Language::Rust,
        Some(Framework::Leptos),
        &[StackMarker::CargoToml],
        &[],
        false,
    );
    assert_eq!(role, ProjectRole::Fullstack);
}
