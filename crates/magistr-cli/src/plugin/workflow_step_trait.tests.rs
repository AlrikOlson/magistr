use super::*;

#[test]
fn workflow_phase_from_str_lossy() {
    assert_eq!(
        WorkflowPhase::from_str_lossy("pre_research"),
        Some(WorkflowPhase::PreResearch)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("post_research"),
        Some(WorkflowPhase::PostResearch)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("pre_execute"),
        Some(WorkflowPhase::PreExecute)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("post_execute"),
        Some(WorkflowPhase::PostExecute)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("pre_verify"),
        Some(WorkflowPhase::PreVerify)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("post_verify"),
        Some(WorkflowPhase::PostVerify)
    );
}

#[test]
fn workflow_phase_from_str_accepts_hyphens() {
    assert_eq!(
        WorkflowPhase::from_str_lossy("pre-research"),
        Some(WorkflowPhase::PreResearch)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("post-verify"),
        Some(WorkflowPhase::PostVerify)
    );
}

#[test]
fn workflow_phase_from_str_case_insensitive() {
    assert_eq!(
        WorkflowPhase::from_str_lossy("PRE_RESEARCH"),
        Some(WorkflowPhase::PreResearch)
    );
    assert_eq!(
        WorkflowPhase::from_str_lossy("Post_Execute"),
        Some(WorkflowPhase::PostExecute)
    );
}

#[test]
fn workflow_phase_from_str_unknown() {
    assert_eq!(WorkflowPhase::from_str_lossy("unknown"), None);
    assert_eq!(WorkflowPhase::from_str_lossy(""), None);
}

#[test]
fn workflow_phase_label_roundtrip() {
    let phases = [
        WorkflowPhase::PreResearch,
        WorkflowPhase::PostResearch,
        WorkflowPhase::PreExecute,
        WorkflowPhase::PostExecute,
        WorkflowPhase::PreVerify,
        WorkflowPhase::PostVerify,
    ];
    for phase in phases {
        assert_eq!(
            WorkflowPhase::from_str_lossy(phase.label()),
            Some(phase),
            "label roundtrip failed for {phase:?}"
        );
    }
}

#[test]
fn workflow_phase_display() {
    assert_eq!(format!("{}", WorkflowPhase::PreResearch), "pre_research");
    assert_eq!(format!("{}", WorkflowPhase::PostVerify), "post_verify");
}

#[test]
fn step_severity_from_str_lossy() {
    assert_eq!(StepSeverity::from_str_lossy("error"), StepSeverity::Error);
    assert_eq!(StepSeverity::from_str_lossy("err"), StepSeverity::Error);
    assert_eq!(
        StepSeverity::from_str_lossy("critical"),
        StepSeverity::Error
    );
    assert_eq!(StepSeverity::from_str_lossy("blocker"), StepSeverity::Error);
    assert_eq!(
        StepSeverity::from_str_lossy("warning"),
        StepSeverity::Warning
    );
    assert_eq!(StepSeverity::from_str_lossy("warn"), StepSeverity::Warning);
    assert_eq!(StepSeverity::from_str_lossy("info"), StepSeverity::Info);
    assert_eq!(StepSeverity::from_str_lossy("anything"), StepSeverity::Info);
}

#[test]
fn step_severity_display() {
    assert_eq!(format!("{}", StepSeverity::Error), "error");
    assert_eq!(format!("{}", StepSeverity::Warning), "warning");
    assert_eq!(format!("{}", StepSeverity::Info), "info");
}

#[test]
fn step_outcome_count_by_severity() {
    let outcome = StepOutcome {
        passed: false,
        step_name: "security-scan".to_string(),
        phase: WorkflowPhase::PreVerify,
        findings: vec![
            StepFinding {
                message: "SQL injection risk".to_string(),
                severity: StepSeverity::Error,
                file: Some("src/db.rs".to_string()),
                line: Some(42),
            },
            StepFinding {
                message: "Unused import".to_string(),
                severity: StepSeverity::Warning,
                file: Some("src/lib.rs".to_string()),
                line: Some(1),
            },
            StepFinding {
                message: "Consider adding docs".to_string(),
                severity: StepSeverity::Info,
                file: None,
                line: None,
            },
            StepFinding {
                message: "XSS vulnerability".to_string(),
                severity: StepSeverity::Error,
                file: Some("src/web.rs".to_string()),
                line: Some(99),
            },
        ],
        summary: "2 errors, 1 warning, 1 info".to_string(),
        duration: std::time::Duration::from_secs(5),
    };

    assert_eq!(outcome.error_count(), 2);
    assert_eq!(outcome.warning_count(), 1);
    assert_eq!(outcome.count_by_severity(StepSeverity::Info), 1);
}

#[test]
fn step_outcome_empty_findings() {
    let outcome = StepOutcome {
        passed: true,
        step_name: "notify".to_string(),
        phase: WorkflowPhase::PostVerify,
        findings: vec![],
        summary: "Notification sent".to_string(),
        duration: std::time::Duration::from_millis(100),
    };

    assert_eq!(outcome.error_count(), 0);
    assert_eq!(outcome.warning_count(), 0);
    assert!(outcome.passed);
}
