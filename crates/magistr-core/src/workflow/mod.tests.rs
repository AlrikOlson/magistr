use super::*;

fn project_json(backend: &str, spec_file: &str) -> String {
    serde_json::json!({
        "project": {"name": "fixture", "description": "", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                  "test_framework": "cargo-nextest", "linter": "clippy", "formatter": "rustfmt"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": spec_file, "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "fixture",
        "roadmap_backend": backend,
    })
    .to_string()
}

fn project(dir: &Path, backend: &str, spec_file: &str) {
    let magistr = dir.join(".magistr");
    std::fs::create_dir_all(&magistr).unwrap();
    std::fs::write(
        magistr.join("project.json"),
        project_json(backend, spec_file),
    )
    .unwrap();
}

#[test]
fn yaml_project_still_requires_its_spec_file() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), "yaml", "ROADMAP.md");
    let err = WorkflowConfig::load(tmp.path()).expect_err("no spec file");
    assert!(err.to_string().contains("Spec file not found"), "{err}");

    std::fs::write(tmp.path().join("ROADMAP.md"), "- [x] one\n- [ ] two\n").unwrap();
    let wf = WorkflowConfig::load(tmp.path()).unwrap();
    assert!(!wf.is_native());
    let summary = wf.spec_summary().unwrap();
    assert_eq!((summary.total, summary.checked), (2, 1));
}

#[test]
fn native_project_loads_without_a_spec_file_and_reads_chunks() {
    use magistr_engine::native::ChunkStatus;
    use magistr_engine::{NativeConfig, ProjectAuthority, WriterRole};

    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), "think-and-ship", ".magistr-roadmap.yml");
    assert!(!tmp.path().join(".magistr-roadmap.yml").exists());

    let wf = WorkflowConfig::load(tmp.path()).expect("native projects need no spec file");
    assert!(wf.is_native());

    let cfg = NativeConfig::from_lookup(tmp.path(), |_| None).with_data_dir(tmp.path().join("tas"));
    let engines = ProjectAuthority::admit_for_project(&cfg, "wf-native".into(), WriterRole::Host)
        .expect("host admitted")
        .open();
    engines
        .roadmap_add_chunk(
            "one".into(),
            "Chunk one".into(),
            String::new(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .unwrap();
    let wf = wf.with_native(Some(engines.reader()));
    let summary = wf.spec_summary().unwrap();
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (1, 0, 1)
    );
    assert_eq!(summary.unchecked_items, vec!["one — Chunk one".to_string()]);
    let prompt = wf.build_prompt().unwrap();
    assert!(prompt.contains("one — Chunk one"), "{prompt}");
}
