use super::*;

#[test]
fn detection_is_an_empty_table_without_project_markers_or_binaries() {
    let root = tempfile::tempdir().unwrap();
    let none = |_: &str| None;
    let no_rust = |_: &Path| None;
    assert!(LanguageServerSpec::detect_with(root.path(), &none, &no_rust).is_empty());
    std::fs::write(root.path().join("tsconfig.json"), "{}").unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package]").unwrap();
    assert!(LanguageServerSpec::detect_with(root.path(), &none, &no_rust).is_empty());
}

#[test]
fn detected_specs_carry_host_curated_argv_and_a_nested_cargo_project_is_found() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    std::fs::create_dir_all(root.path().join("rust")).unwrap();
    std::fs::write(root.path().join("rust/Cargo.toml"), "[package]").unwrap();
    std::fs::create_dir_all(root.path().join("node_modules/decoy")).unwrap();
    std::fs::write(root.path().join("node_modules/decoy/Cargo.toml"), "x").unwrap();
    let lookup = |bin: &str| (bin == "typescript-language-server").then(|| PathBuf::from("/tls"));
    let rust = |dir: &Path| {
        assert!(dir.ends_with("rust"), "{}", dir.display());
        Some(PathBuf::from("/ra"))
    };
    let specs = LanguageServerSpec::detect_with(root.path(), &lookup, &rust);
    assert_eq!(
        specs,
        vec![
            LanguageServerSpec {
                language_id: "typescript".into(),
                bin: "/tls".into(),
                args: vec!["--stdio".into()],
            },
            LanguageServerSpec {
                language_id: "rust".into(),
                bin: "/ra".into(),
                args: Vec::new(),
            },
        ]
    );
    let wire = WorkerSubprocessSpec::from(&specs[0]);
    assert_eq!(wire.args, vec!["--stdio"]);
    // The real probes never fail; a root without markers is simply empty.
    assert!(LanguageServerSpec::detect(tempfile::tempdir().unwrap().path()).is_empty());
}
