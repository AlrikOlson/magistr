use crate::cost::bloated_file::BloatedFile;
use crate::cost::collect_hints::collect_hints;
use crate::cost::context_bloat_analysis::ContextBloatAnalysis;
use crate::cost::detect_context_bloat::detect_context_bloat;
use crate::cost::detect_retry_loop::detect_retry_loop;
use crate::cost::hint_severity::HintSeverity;
use crate::cost::pruning_suggestion::pruning_suggestion;
use crate::cost::retry_loop_analysis::RetryLoopAnalysis;
use crate::cost::retry_loop_hint::retry_loop_hint;
use crate::cost::turn_outcome::TurnOutcome;

// --- Context bloat tests ---

#[test]
fn no_bloat_when_no_files() {
    let analysis = detect_context_bloat(&[], 500, 3, 0.000003);
    assert!(analysis.bloated_files.is_empty());
    assert_eq!(analysis.total_reclaimable_tokens, 0);
}

#[test]
fn no_bloat_when_files_recently_used() {
    let files = vec![BloatedFile {
        path: "src/main.rs".to_string(),
        estimated_tokens: 2000,
        turns_since_referenced: 1, // recently used
    }];
    let analysis = detect_context_bloat(&files, 500, 3, 0.000003);
    assert!(analysis.bloated_files.is_empty());
}

#[test]
fn no_bloat_when_files_small() {
    let files = vec![BloatedFile {
        path: "src/lib.rs".to_string(),
        estimated_tokens: 100, // small file
        turns_since_referenced: 10,
    }];
    let analysis = detect_context_bloat(&files, 500, 3, 0.000003);
    assert!(analysis.bloated_files.is_empty());
}

#[test]
fn detects_bloated_file() {
    let files = vec![
        BloatedFile {
            path: "src/big_unused.rs".to_string(),
            estimated_tokens: 5000,
            turns_since_referenced: 5,
        },
        BloatedFile {
            path: "src/active.rs".to_string(),
            estimated_tokens: 3000,
            turns_since_referenced: 1,
        },
    ];
    let analysis = detect_context_bloat(&files, 500, 3, 0.000003);
    assert_eq!(analysis.bloated_files.len(), 1);
    assert_eq!(analysis.bloated_files[0].path, "src/big_unused.rs");
    assert_eq!(analysis.total_reclaimable_tokens, 5000);
}

#[test]
fn savings_calculation() {
    let files = vec![BloatedFile {
        path: "src/huge.rs".to_string(),
        estimated_tokens: 10000,
        turns_since_referenced: 5,
    }];
    // $3/1M tokens = $0.000003/token
    let analysis = detect_context_bloat(&files, 500, 3, 0.000003);
    assert!((analysis.estimated_savings_per_turn - 0.03).abs() < f64::EPSILON);
}

#[test]
fn pruning_suggestion_none_when_no_bloat() {
    let analysis = ContextBloatAnalysis {
        bloated_files: vec![],
        total_reclaimable_tokens: 0,
        estimated_savings_per_turn: 0.0,
    };
    assert!(pruning_suggestion(&analysis).is_none());
}

#[test]
fn pruning_suggestion_formats_correctly() {
    let analysis = ContextBloatAnalysis {
        bloated_files: vec![
            BloatedFile {
                path: "a.rs".to_string(),
                estimated_tokens: 1000,
                turns_since_referenced: 5,
            },
            BloatedFile {
                path: "b.rs".to_string(),
                estimated_tokens: 2000,
                turns_since_referenced: 4,
            },
            BloatedFile {
                path: "c.rs".to_string(),
                estimated_tokens: 1500,
                turns_since_referenced: 6,
            },
        ],
        total_reclaimable_tokens: 4500,
        estimated_savings_per_turn: 0.12,
    };

    let hint = pruning_suggestion(&analysis).expect("should produce hint");
    assert_eq!(hint.title, "Context bloat detected");
    assert!(hint.description.contains("3 unused files"));
    assert!(hint.description.contains("$0.12/turn"));
    assert_eq!(hint.severity, HintSeverity::Critical);
}

#[test]
fn pruning_singular_file() {
    let analysis = ContextBloatAnalysis {
        bloated_files: vec![BloatedFile {
            path: "a.rs".to_string(),
            estimated_tokens: 1000,
            turns_since_referenced: 5,
        }],
        total_reclaimable_tokens: 1000,
        estimated_savings_per_turn: 0.01,
    };

    let hint = pruning_suggestion(&analysis).expect("should produce hint");
    assert!(hint.description.contains("1 unused file "));
    assert!(!hint.description.contains("files"));
}

// --- Retry loop tests ---

#[test]
fn no_loop_when_too_few_turns() {
    let outcomes = vec![TurnOutcome::Failure("err".to_string())];
    let analysis = detect_retry_loop(&outcomes, 3, 3, 5000);
    assert!(!analysis.is_looping);
}

#[test]
fn no_loop_when_successes() {
    let outcomes = vec![
        TurnOutcome::Success,
        TurnOutcome::Success,
        TurnOutcome::Success,
    ];
    let analysis = detect_retry_loop(&outcomes, 3, 3, 5000);
    assert!(!analysis.is_looping);
}

#[test]
fn detects_retry_loop() {
    let outcomes = vec![
        TurnOutcome::Success,
        TurnOutcome::Failure("compile_error:E0308".to_string()),
        TurnOutcome::Failure("compile_error:E0308".to_string()),
        TurnOutcome::Failure("compile_error:E0308".to_string()),
    ];
    let analysis = detect_retry_loop(&outcomes, 4, 3, 5000);
    assert!(analysis.is_looping);
    assert_eq!(
        analysis.repeated_error.as_deref(),
        Some("compile_error:E0308")
    );
    assert_eq!(analysis.consecutive_failures, 3);
    // 3 failures, waste = (3-1) * 5000 = 10000
    assert_eq!(analysis.estimated_wasted_tokens, 10000);
}

#[test]
fn no_loop_when_different_errors() {
    let outcomes = vec![
        TurnOutcome::Failure("err_a".to_string()),
        TurnOutcome::Failure("err_b".to_string()),
        TurnOutcome::Failure("err_c".to_string()),
    ];
    let analysis = detect_retry_loop(&outcomes, 3, 3, 5000);
    assert!(!analysis.is_looping);
    assert_eq!(analysis.consecutive_failures, 1);
}

#[test]
fn loop_broken_by_success() {
    let outcomes = vec![
        TurnOutcome::Failure("err".to_string()),
        TurnOutcome::Failure("err".to_string()),
        TurnOutcome::Success,
        TurnOutcome::Failure("err".to_string()),
    ];
    let analysis = detect_retry_loop(&outcomes, 4, 2, 5000);
    assert!(!analysis.is_looping);
    assert_eq!(analysis.consecutive_failures, 1);
}

#[test]
fn retry_hint_none_when_not_looping() {
    let analysis = RetryLoopAnalysis {
        is_looping: false,
        repeated_error: None,
        consecutive_failures: 1,
        estimated_wasted_tokens: 0,
    };
    assert!(retry_loop_hint(&analysis).is_none());
}

#[test]
fn retry_hint_warning_severity() {
    let analysis = RetryLoopAnalysis {
        is_looping: true,
        repeated_error: Some("test_failure".to_string()),
        consecutive_failures: 3,
        estimated_wasted_tokens: 10000,
    };
    let hint = retry_loop_hint(&analysis).expect("should produce hint");
    assert_eq!(hint.title, "Retry loop detected");
    assert!(hint.description.contains("3 consecutive times"));
    assert!(hint.description.contains("test_failure"));
    assert_eq!(hint.severity, HintSeverity::Warning);
}

#[test]
fn retry_hint_critical_at_5_failures() {
    let analysis = RetryLoopAnalysis {
        is_looping: true,
        repeated_error: Some("err".to_string()),
        consecutive_failures: 5,
        estimated_wasted_tokens: 20000,
    };
    let hint = retry_loop_hint(&analysis).expect("should produce hint");
    assert_eq!(hint.severity, HintSeverity::Critical);
}

// --- collect_hints tests ---

#[test]
fn collect_hints_empty_when_nothing() {
    let hints = collect_hints(None, None);
    assert!(hints.is_empty());
}

#[test]
fn collect_hints_combines_and_sorts() {
    let bloat = ContextBloatAnalysis {
        bloated_files: vec![BloatedFile {
            path: "a.rs".to_string(),
            estimated_tokens: 1000,
            turns_since_referenced: 5,
        }],
        total_reclaimable_tokens: 1000,
        estimated_savings_per_turn: 0.01, // Info severity
    };
    let retry = RetryLoopAnalysis {
        is_looping: true,
        repeated_error: Some("err".to_string()),
        consecutive_failures: 5, // Critical severity
        estimated_wasted_tokens: 20000,
    };

    let hints = collect_hints(Some(&bloat), Some(&retry));
    assert_eq!(hints.len(), 2);
    // Critical first
    assert_eq!(hints[0].severity, HintSeverity::Critical);
    assert_eq!(hints[1].severity, HintSeverity::Info);
}

// --- HintSeverity tests ---

#[test]
fn severity_ordering() {
    assert!(HintSeverity::Critical > HintSeverity::Warning);
    assert!(HintSeverity::Warning > HintSeverity::Info);
}

#[test]
fn severity_display() {
    assert_eq!(format!("{}", HintSeverity::Info), "info");
    assert_eq!(format!("{}", HintSeverity::Warning), "warning");
    assert_eq!(format!("{}", HintSeverity::Critical), "critical");
}
