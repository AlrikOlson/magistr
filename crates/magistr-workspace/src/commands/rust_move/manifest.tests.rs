use super::*;

const MANIFEST: &str = "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n\n[workspace]\nmembers = [\"crates/x\"]\n\n[lib]\npath = \"src/lib_root.rs\"\n\n[[bin]]\nname = \"cli\"\npath = \"./src/tools/cli.rs\"\n\n[dependencies]\nother = { path = \"../other\" }\n";

#[test]
fn manifest_target_paths_reads_lib_and_bin_paths_normalized_and_ignores_dependencies() {
    assert_eq!(
        manifest_target_paths(MANIFEST),
        vec!["src/lib_root.rs".to_owned(), "src/tools/cli.rs".to_owned()]
    );
}

#[test]
fn rewrite_manifest_paths_rewrites_only_the_matching_target_entry_and_keeps_the_file_well_formed() {
    let (content, count) = rewrite_manifest_paths(MANIFEST, "src/tools/cli.rs", "src/bin/cli.rs");
    assert_eq!(count, 1);
    assert!(content.contains("path = \"src/bin/cli.rs\""));
    assert!(content.contains("members = [\"crates/x\"]"));
    assert!(content.contains("other = { path = \"../other\" }"));
    assert!(toml::from_str::<toml::Value>(&content).is_ok());
    assert_eq!(
        rewrite_manifest_paths(MANIFEST, "src/nothing.rs", "x.rs"),
        (MANIFEST.to_owned(), 0)
    );
}
