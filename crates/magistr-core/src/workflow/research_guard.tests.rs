use super::*;

use crate::workflow::guards::GuardContext;
use tempfile::TempDir;

fn temp_project() -> (TempDir, GuardContext) {
    let dir = TempDir::new().unwrap();
    let project_dir = dir.path().to_string_lossy().to_string();
    let spec_path = dir.path().join("ROADMAP.md").to_string_lossy().to_string();
    let ctx = GuardContext::new(
        project_dir,
        spec_path,
        magistr_engine::ShipGuardState::default(),
    );
    (dir, ctx)
}

// ── WorkflowGuard trait (session hooks are no-ops) ──────────────────────────

#[test]
fn pre_session_is_ok() {
    let (_dir, ctx) = temp_project();
    let guard = ResearchGuard;
    let result = guard.pre_session(&ctx);
    assert!(result.is_ok());
}

#[test]
fn name_is_research() {
    assert_eq!(ResearchGuard.name(), "research");
}
