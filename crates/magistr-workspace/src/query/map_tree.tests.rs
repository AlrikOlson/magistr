use super::*;

fn entries(paths: &[&str]) -> Vec<SourceEntry> {
    paths
        .iter()
        .map(|path| SourceEntry {
            path: (*path).into(),
            is_dir: false,
            junk: false,
        })
        .collect()
}

#[test]
fn tree_is_breadth_first_with_recursive_counts_and_scoped_census() {
    let files = entries(&[
        "src/z/deep.rs",
        "src/lib.rs",
        "Cargo.toml",
        "tests/a.rs",
        "README.md",
    ]);
    let tree = MapTree::build(&files, "", 2);
    assert_eq!(
        tree.rows.iter().map(MapRow::path).collect::<Vec<_>>(),
        [
            "src",
            "tests",
            "Cargo.toml",
            "README.md",
            "src/z",
            "src/lib.rs",
            "tests/a.rs"
        ]
    );
    assert!(matches!(
        &tree.rows[0],
        MapRow::Dir {
            files: 2,
            dirs: 1,
            ..
        }
    ));
    assert_eq!(tree.files, 5);
    assert_eq!(tree.dirs, 3);
    assert_eq!(tree.languages["rust"], 3);
    let scoped = MapTree::build(&files, "src", 1);
    assert_eq!(scoped.files, 2);
    assert_eq!(
        scoped.rows.iter().map(MapRow::path).collect::<Vec<_>>(),
        ["src/z", "src/lib.rs"]
    );
    assert_eq!(scoped.languages.len(), 1);
}

#[test]
fn census_uses_only_real_basename_extensions() {
    for path in ["some.dir/README", ".gitignore", "trail."] {
        assert_eq!(language_of(path), None);
    }
    assert_eq!(language_of("src/MODULE.RS"), Some("rust".into()));
    assert_eq!(language_of("data.custom"), Some("custom".into()));
}
