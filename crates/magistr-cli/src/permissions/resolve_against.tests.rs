use super::*;

use std::path::{Path, PathBuf};

#[test]
fn a_relative_path_is_joined_to_the_root() {
    let out = resolve_against(Some(Path::new("/project")), Path::new("ROADMAP.md"));
    assert_eq!(out, PathBuf::from("/project/ROADMAP.md"));
}

#[test]
fn dot_and_dot_dot_components_are_resolved_lexically() {
    let out = resolve_against(
        Some(Path::new("/project")),
        Path::new("./src/../.magistr/./roadmap.yaml"),
    );
    assert_eq!(out, PathBuf::from("/project/.magistr/roadmap.yaml"));
}

#[test]
fn an_absolute_path_ignores_the_root() {
    let out = resolve_against(
        Some(Path::new("/project")),
        Path::new("/elsewhere/../x/y.md"),
    );
    assert_eq!(out, PathBuf::from("/x/y.md"));
}

#[test]
fn without_a_root_a_relative_path_stays_relative_but_normalized() {
    let out = resolve_against(None, Path::new("./.magistr/../.magistr/roadmap.yaml"));
    assert_eq!(out, PathBuf::from(".magistr/roadmap.yaml"));
}

#[test]
fn a_leading_dot_dot_on_a_relative_path_is_kept() {
    let out = resolve_against(None, Path::new("../sibling/file.rs"));
    assert_eq!(out, PathBuf::from("../sibling/file.rs"));
}

#[test]
fn dot_dot_cannot_climb_above_an_absolute_root() {
    let out = resolve_against(
        Some(Path::new("/project")),
        Path::new("../../../no-such-root/x.md"),
    );
    assert_eq!(out, PathBuf::from("/no-such-root/x.md"));
}

#[test]
fn an_existing_path_is_canonicalized_so_symlinked_roots_agree() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let file = dir.path().join("ROADMAP.md");
    std::fs::write(&file, "x").expect("write");
    let canonical = std::fs::canonicalize(&file).expect("canonical");
    // The temp dir on macOS is /var/... → /private/var/...: a relative
    // target resolved against the uncanonical root must still land on
    // the canonical file.
    let out = resolve_against(Some(dir.path()), Path::new("ROADMAP.md"));
    assert_eq!(out, canonical);
}

#[test]
fn a_path_that_does_not_exist_yet_is_still_resolved() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let out = resolve_against(Some(dir.path()), Path::new("new/./file.rs"));
    assert!(out.ends_with("new/file.rs"), "{out:?}");
    assert!(out.is_absolute());
}
