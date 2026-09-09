use super::*;
use std::fs;

fn make_temp_repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    // Initialize as a git repo so .gitignore is respected
    fs::create_dir(tmp.path().join(".git")).unwrap();
    tmp
}

#[test]
fn scan_finds_cargo_toml_at_root() {
    let tmp = make_temp_repo();
    fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    let markers = scan_directory(tmp.path());
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].marker, StackMarker::CargoToml);
    assert_eq!(markers[0].depth, 1); // ignore crate reports files at depth 1 for root
}

#[test]
fn scan_finds_nested_package_json() {
    let tmp = make_temp_repo();
    fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
    fs::write(tmp.path().join("apps/web/package.json"), "{}").unwrap();

    let markers = scan_directory(tmp.path());
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].marker, StackMarker::PackageJson);
    assert_eq!(markers[0].dir, PathBuf::from("apps/web"));
}

#[test]
fn scan_skips_node_modules() {
    let tmp = make_temp_repo();
    fs::write(tmp.path().join("package.json"), "{}").unwrap();
    fs::create_dir_all(tmp.path().join("node_modules/some-dep")).unwrap();
    fs::write(tmp.path().join("node_modules/some-dep/package.json"), "{}").unwrap();

    let markers = scan_directory(tmp.path());
    assert_eq!(markers.len(), 1, "should skip node_modules");
    assert_eq!(markers[0].dir, PathBuf::from(""));
}

#[test]
fn scan_skips_target_directory() {
    let tmp = make_temp_repo();
    fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
    fs::create_dir_all(tmp.path().join("target/debug")).unwrap();
    fs::write(tmp.path().join("target/debug/build.gradle"), "").unwrap();

    let markers = scan_directory(tmp.path());
    assert_eq!(markers.len(), 1, "should skip target/");
}

#[test]
fn scan_respects_gitignore() {
    let tmp = make_temp_repo();
    fs::write(tmp.path().join(".gitignore"), "ignored-dir/\n").unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
    fs::create_dir_all(tmp.path().join("ignored-dir")).unwrap();
    fs::write(tmp.path().join("ignored-dir/package.json"), "{}").unwrap();

    let markers = scan_directory(tmp.path());
    assert_eq!(markers.len(), 1, "should respect .gitignore");
    assert_eq!(markers[0].marker, StackMarker::CargoToml);
}

#[test]
fn scan_finds_csproj_glob() {
    let tmp = make_temp_repo();
    fs::write(tmp.path().join("MyApp.csproj"), "<Project />").unwrap();

    let markers = scan_directory(tmp.path());
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].marker, StackMarker::Csproj);
}

#[test]
fn scan_sorted_by_depth_then_priority() {
    let tmp = make_temp_repo();
    // Root has workspace marker
    fs::write(tmp.path().join("nx.json"), "{}").unwrap();
    // Root also has package.json
    fs::write(tmp.path().join("package.json"), "{}").unwrap();
    // Nested project
    fs::create_dir_all(tmp.path().join("apps/api")).unwrap();
    fs::write(tmp.path().join("apps/api/package.json"), "{}").unwrap();

    let markers = scan_directory(tmp.path());
    assert!(markers.len() >= 3);
    // Root markers should come first (depth 1)
    assert!(markers[0].depth <= markers.last().unwrap().depth);
    // Within same depth, lower priority (more specific) should come first
    let root_markers: Vec<_> = markers.iter().filter(|m| m.depth == 1).collect();
    if root_markers.len() >= 2 {
        assert!(root_markers[0].marker.priority() <= root_markers[1].marker.priority());
    }
}

#[test]
fn unique_project_dirs_deduplicates() {
    let tmp = make_temp_repo();
    // A directory with both Cargo.toml and package.json (e.g., tauri project)
    fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
    fs::write(tmp.path().join("package.json"), "{}").unwrap();

    let markers = scan_directory(tmp.path());
    let dirs = unique_project_dirs(&markers);
    assert_eq!(dirs.len(), 1, "same directory should be merged");
    assert!(dirs[0].1.len() >= 2, "both markers should be present");
}
