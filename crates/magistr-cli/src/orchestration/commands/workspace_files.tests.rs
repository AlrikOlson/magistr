use super::*;

#[test]
fn lists_files_relative_sorted_and_skips_hidden_and_build_trees() {
    let dir = tempfile::tempdir().expect("a tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src/cockpit")).expect("mkdir");
    std::fs::create_dir_all(root.join("target/debug")).expect("mkdir");
    std::fs::create_dir_all(root.join(".git")).expect("mkdir");
    for file in [
        "README.md",
        "src/main.rs",
        "src/cockpit/prompt.rs",
        "target/debug/junk.o",
        ".gitignore",
        ".git/HEAD",
    ] {
        std::fs::write(root.join(file), "x").expect("write");
    }
    assert_eq!(
        workspace_files(root),
        ["README.md", "src/cockpit/prompt.rs", "src/main.rs"],
        "relative, sorted, no hidden entries, no build tree"
    );
    assert_eq!(
        workspace_files(&root.join("missing")),
        Vec::<String>::new(),
        "an unreadable root is an empty list"
    );
}
