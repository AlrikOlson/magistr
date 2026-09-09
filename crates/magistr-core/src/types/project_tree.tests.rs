use super::*;

#[test]
fn empty_tree() {
    let tree = ProjectTree::new(PathBuf::from("/tmp/repo"));
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
    assert_eq!(tree.topology, Topology::SingleProject);
    assert_eq!(tree.workspace, Workspace::None);
}

#[test]
fn find_by_path_works() {
    let mut tree = ProjectTree::new(PathBuf::from("/tmp/repo"));
    tree.projects.push(DetectedProject {
        path: PathBuf::from("apps/web"),
        language: Language::TypeScript,
        framework: Some(Framework::React),
        role: ProjectRole::Frontend,
        markers: vec![StackMarker::PackageJson],
        metadata: ProjectMetadata::default(),
        parent: None,
        children: vec![],
    });
    assert!(
        tree.find_by_path(std::path::Path::new("apps/web"))
            .is_some()
    );
    assert!(
        tree.find_by_path(std::path::Path::new("apps/api"))
            .is_none()
    );
}

#[test]
fn path_index_maps_correctly() {
    let mut tree = ProjectTree::new(PathBuf::from("/tmp/repo"));
    tree.projects.push(DetectedProject {
        path: PathBuf::from("apps/web"),
        language: Language::TypeScript,
        framework: None,
        role: ProjectRole::Frontend,
        markers: vec![],
        metadata: ProjectMetadata::default(),
        parent: None,
        children: vec![],
    });
    tree.projects.push(DetectedProject {
        path: PathBuf::from("apps/api"),
        language: Language::TypeScript,
        framework: None,
        role: ProjectRole::Backend,
        markers: vec![],
        metadata: ProjectMetadata::default(),
        parent: None,
        children: vec![],
    });
    let idx = tree.path_index();
    assert_eq!(idx.len(), 2);
    assert_eq!(idx[std::path::Path::new("apps/web")], 0);
    assert_eq!(idx[std::path::Path::new("apps/api")], 1);
}
