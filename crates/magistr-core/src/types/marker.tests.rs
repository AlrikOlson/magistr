use super::*;

#[test]
fn from_filename_exact_matches() {
    assert_eq!(
        StackMarker::from_filename("Cargo.toml"),
        Some(StackMarker::CargoToml)
    );
    assert_eq!(
        StackMarker::from_filename("package.json"),
        Some(StackMarker::PackageJson)
    );
    assert_eq!(
        StackMarker::from_filename("go.mod"),
        Some(StackMarker::GoMod)
    );
    assert_eq!(
        StackMarker::from_filename("nx.json"),
        Some(StackMarker::NxJson)
    );
}

#[test]
fn from_filename_glob_matches() {
    assert_eq!(
        StackMarker::from_filename("MyApp.csproj"),
        Some(StackMarker::Csproj)
    );
    assert_eq!(
        StackMarker::from_filename("Solution.sln"),
        Some(StackMarker::Sln)
    );
}

#[test]
fn from_filename_docker_compose_variants() {
    assert_eq!(
        StackMarker::from_filename("docker-compose.yml"),
        Some(StackMarker::DockerCompose)
    );
    assert_eq!(
        StackMarker::from_filename("docker-compose.yaml"),
        Some(StackMarker::DockerCompose)
    );
    assert_eq!(
        StackMarker::from_filename("compose.yml"),
        Some(StackMarker::DockerCompose)
    );
    assert_eq!(
        StackMarker::from_filename("compose.yaml"),
        Some(StackMarker::DockerCompose)
    );
}

#[test]
fn from_filename_unknown() {
    assert_eq!(StackMarker::from_filename("README.md"), None);
    assert_eq!(StackMarker::from_filename(""), None);
}

#[test]
fn workspace_tools_are_not_project_boundaries() {
    assert!(StackMarker::NxJson.is_workspace_tool());
    assert!(!StackMarker::NxJson.is_project_boundary());
    assert!(StackMarker::TurboJson.is_workspace_tool());
    assert!(StackMarker::PnpmWorkspaceYaml.is_workspace_tool());
}

#[test]
fn project_markers_are_boundaries() {
    assert!(StackMarker::CargoToml.is_project_boundary());
    assert!(StackMarker::PackageJson.is_project_boundary());
    assert!(StackMarker::GoMod.is_project_boundary());
    assert!(!StackMarker::DockerCompose.is_project_boundary());
}

#[test]
fn priority_ordering() {
    // Language manifests should rank higher (lower number) than workspace tools
    assert!(StackMarker::CargoToml.priority() < StackMarker::NxJson.priority());
    assert!(StackMarker::PackageJson.priority() < StackMarker::LernaJson.priority());
}

#[test]
fn all_variants_covered() {
    // Every variant should have a filename and be matchable
    for marker in StackMarker::ALL {
        let filename = marker.filename();
        assert!(!filename.is_empty(), "{marker:?} has empty filename");
        // Non-glob markers should round-trip through from_filename
        if !marker.is_glob() {
            assert_eq!(
                StackMarker::from_filename(filename),
                Some(*marker),
                "{marker:?} doesn't round-trip"
            );
        }
    }
}
