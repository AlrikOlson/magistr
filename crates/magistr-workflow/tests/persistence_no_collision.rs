//! Regression test: Magistr workflow persistence must not
//! share an on-disk path when configured against the same data dir, or
//! they will silently clobber each other's `<project>.json` files.

use std::path::PathBuf;

use tempfile::TempDir;

use magistr_workflow::decision::persistence::Persistence as DecisionPersistence;
use magistr_workflow::infra::PersistenceConfig as DecisionPersistenceConfig;
use magistr_workflow::run::persistence::Persistence as RunPersistence;
use magistr_workflow::run::persistence::PersistenceConfig as RunPersistenceConfig;

#[test]
fn workflow_persistence_live_in_disjoint_subdirs() {
    let tmp = TempDir::new().unwrap();
    let data_dir: PathBuf = tmp.path().to_path_buf();

    let decision = DecisionPersistence::for_project(
        &DecisionPersistenceConfig {
            enabled: true,
            data_dir: data_dir.clone(),
        },
        "project",
    );
    let run = RunPersistence::new(&RunPersistenceConfig {
        enabled: true,
        data_dir: data_dir.clone(),
    });

    // decision exposes its documents_dir; run does not, so we reconstruct
    // the expected path and assert both physical directories were
    // created (the constructors mkdir on demand when enabled=true).
    let decision_dir = decision.documents_dir();
    let run_dir = data_dir.join("run").join("sessions");

    assert!(
        decision_dir.ends_with("decisions/sessions"),
        "decision documents_dir should end with `decisions/sessions`, got {}",
        decision_dir.display()
    );
    assert!(
        run_dir.exists(),
        "run sessions dir should be created at {}",
        run_dir.display()
    );
    assert_ne!(
        decision_dir,
        run_dir.as_path(),
        "Magistr workflow persistence must not share a directory; \
         decision_dir={} run_dir={}",
        decision_dir.display(),
        run_dir.display()
    );

    // Keep run borrowed so the test exercises both handles, not just
    // decision. The constructor side effects (mkdir) are the load-bearing
    // observation here.
    let _ = &run;
}
