use super::*;

#[test]
fn quarantine_identity_changes_when_a_file_is_repaired() {
    let input = |source: &str| WorkerRequest::Parse {
        path: "same.rs".to_owned(),
        language: "rust".to_owned(),
        source: source.to_owned(),
    };
    assert_eq!(input("bad").input_key(), input("bad").input_key());
    assert_ne!(input("bad").input_key(), input("fixed").input_key());
    assert_eq!(WorkerRequest::Ping.input_key(), None);
    let long = WorkerRequest::Parse {
        path: "path".repeat(1000),
        language: "rust".to_owned(),
        source: "content".to_owned(),
    };
    assert_eq!(
        long.input_key().expect("parser input identity").len(),
        120 + 1 + 64
    );
}

#[test]
fn parser_and_rewrite_use_the_native_engine() {
    let parsed = WorkerRequest::Parse {
        path: "input.rs".to_owned(),
        language: "rust".to_owned(),
        source: "pub fn answer() -> u32 { 42 }".to_owned(),
    }
    .execute()
    .expect("native parse completes");
    assert!(
        matches!(parsed, WorkerResponse::Parse { has_error: false, node_count, .. } if node_count > 5)
    );
    let rewritten = WorkerRequest::Rewrite {
        path: "input.js".to_owned(),
        language: "javascript".to_owned(),
        source: "console.log(answer);".to_owned(),
        pattern: "console.log($A)".to_owned(),
        replacement: "console.debug($A)".to_owned(),
    }
    .execute()
    .expect("native structural rewrite completes");
    assert_eq!(
        rewritten,
        WorkerResponse::Rewrite {
            output: "console.debug(answer);".to_owned(),
            match_count: 1,
        }
    );
}

#[test]
fn extraction_and_search_keep_source_coordinates_and_owned_results() {
    let source = "use std::fmt::Debug;\n\npub fn answer() -> u32 { 42 }\n";
    let result = WorkerRequest::Extract {
        path: "input.rs".to_owned(),
        language: "rust".to_owned(),
        source: source.to_owned(),
    }
    .execute()
    .expect("native extraction completes");
    let WorkerResponse::Extract {
        symbols, imports, ..
    } = &result
    else {
        panic!("extraction must return the extraction variant");
    };
    assert_eq!(imports, &["std::fmt::Debug"]);
    assert!(symbols.iter().any(|symbol| symbol.name == "answer"
        && symbol.start_line == 3
        && symbol.kind == "function"));
    let wire = serde_json::to_vec(&result).expect("owned extraction serializes");
    assert_eq!(
        serde_json::from_slice::<WorkerResponse>(&wire).expect("owned extraction deserializes"),
        result
    );
    let found = WorkerRequest::Search {
        path: "input.rs".to_owned(),
        pattern: "ANSWER".to_owned(),
        haystack: source.to_owned(),
        case_insensitive: true,
    }
    .execute()
    .expect("native search completes");
    assert!(
        matches!(found, WorkerResponse::Search { hits } if hits.len() == 1 && hits[0].line == 3 && hits[0].start == 7)
    );
}

#[test]
fn structural_search_preserves_ast_match_coordinates() {
    let result = WorkerRequest::Structural {
        path: "input.js".to_owned(),
        language: "javascript".to_owned(),
        source: "keep();\nconsole.log(answer);\n".to_owned(),
        pattern: "console.log($A)".to_owned(),
    }
    .execute()
    .expect("native structural search completes");
    assert!(
        matches!(result, WorkerResponse::Structural { hits } if hits.len() == 1 && hits[0].line == 2 && hits[0].kind == "call_expression")
    );
    assert_eq!(
        WorkerRequest::Ping.execute().expect("startup self-test"),
        WorkerResponse::Pong
    );
}

#[test]
fn invalid_pattern_and_unknown_language_have_answerable_codes() {
    let bad = WorkerRequest::Search {
        path: "input.txt".to_owned(),
        pattern: "(".to_owned(),
        haystack: "content".to_owned(),
        case_insensitive: false,
    }
    .execute()
    .expect_err("bad regex is refused");
    assert!(matches!(bad, WorkerError::Remote { code, .. } if code == "PATTERN_INVALID"));
    let unknown = WorkerRequest::Parse {
        path: "input.unknown".to_owned(),
        language: "unsupported".to_owned(),
        source: "content".to_owned(),
    }
    .execute()
    .expect_err("unknown parser is refused");
    assert!(matches!(unknown, WorkerError::Remote { code, .. } if code == "LANGUAGE_UNAVAILABLE"));
}

#[cfg(feature = "sandbox")]
#[test]
fn sandbox_dispatch_preserves_guest_limits_and_capabilities() {
    use crate::worker::{WorkerGuestEnd, WorkerPolicy};
    let run = |guest: &[u8], export: &str, capabilities: Vec<String>| {
        WorkerRequest::Component {
            guest: guest.to_vec(),
            export: export.to_owned(),
            args: Vec::new(),
            capabilities,
            policy: WorkerPolicy {
                fuel: 100_000,
                memory_cap_bytes: 10 * 65536,
            },
        }
        .execute()
        .expect("guest outcome is typed data")
    };
    let spin = include_bytes!("../../../../subsystems/workspace/conformance/spin.wat");
    let hog = include_bytes!("../../../../subsystems/workspace/conformance/hog.wat");
    let observe = include_bytes!("../../../../subsystems/workspace/conformance/observe.wat");
    assert!(matches!(
        run(spin, "spin", Vec::new()),
        WorkerResponse::Component {
            end: WorkerGuestEnd::OutOfFuel,
            ..
        }
    ));
    assert!(matches!(
        run(hog, "hog", Vec::new()),
        WorkerResponse::Component {
            end: WorkerGuestEnd::MemoryCap,
            ..
        }
    ));
    assert!(matches!(
        run(observe, "run", Vec::new()),
        WorkerResponse::Component {
            end: WorkerGuestEnd::CapDenied(_),
            fuel_used: 0,
            ..
        }
    ));
    assert!(
        matches!(run(observe, "run", vec!["observe".to_owned()]), WorkerResponse::Component { end: WorkerGuestEnd::Ok, observations, .. } if observations == [7, 11, 13])
    );
}

#[cfg(feature = "sandbox")]
#[test]
fn budget_admission_only_clamps_downward() {
    use crate::worker::WorkerPolicy;
    let small = WorkerPolicy {
        fuel: 19,
        memory_cap_bytes: 23,
    }
    .admitted()
    .expect("positive policy");
    assert_eq!((small.fuel, small.memory_cap_bytes), (19, 23));
    let large = WorkerPolicy {
        fuel: u64::MAX,
        memory_cap_bytes: usize::MAX,
    }
    .admitted()
    .expect("clamped policy");
    assert_eq!(
        (large.fuel, large.memory_cap_bytes),
        (1_000_000_000, 256 * 1024 * 1024)
    );
    assert!(
        WorkerPolicy {
            fuel: 0,
            memory_cap_bytes: 23
        }
        .admitted()
        .is_err()
    );
}

#[cfg(feature = "sandbox")]
#[test]
fn wasi_keeps_output_capture_and_no_preopen_authority() {
    use crate::worker::{WorkerGuestEnd, WorkerPolicy};
    let run = |guest: &[u8]| {
        WorkerRequest::Wasi {
            guest: guest.to_vec(),
            argv: vec!["probe".to_owned()],
            policy: WorkerPolicy {
                fuel: 1_000_000,
                memory_cap_bytes: 10 * 65536,
            },
        }
        .execute()
        .expect("WASI conditions are data")
    };
    let hello = include_bytes!("../../../../subsystems/workspace/conformance/hello_wasi.wat");
    let probe = include_bytes!("../../../../subsystems/workspace/conformance/preopen_probe.wat");
    assert!(
        matches!(run(hello), WorkerResponse::Wasi { end: WorkerGuestEnd::Ok, stdout, stderr, .. } if stdout == "jailed" && stderr.is_empty())
    );
    assert!(matches!(
        run(probe),
        WorkerResponse::Wasi {
            end: WorkerGuestEnd::GuestExit(8),
            exit_code: Some(8),
            ..
        }
    ));
}

#[cfg(feature = "lang-pack")]
#[test]
fn worker_registration_preserves_bundled_grammar_shadow_refusal() {
    let result = WorkerRequest::RegisterLanguage {
        id: "rust".to_owned(),
        extensions: vec!["rs".to_owned()],
        grammar_wasm: Vec::new(),
        tag_queries: String::new(),
        blessed: true,
    }
    .execute()
    .expect_err("a pack cannot shadow a bundled grammar");
    assert!(
        matches!(result, WorkerError::Remote { code, message } if code == "LANGUAGE_UNAVAILABLE" && message.contains("shadows a bundled"))
    );
}

#[cfg(feature = "sandbox")]
#[test]
fn ranking_and_runner_lanes_dispatch_the_retained_typed_guests() {
    use crate::worker::{WorkerGuestEnd, WorkerPolicy, WorkerRankingHit};
    let policy = WorkerPolicy {
        fuel: 200_000_000,
        memory_cap_bytes: 32 * 1024 * 1024,
    };
    let ranking = include_bytes!(
        "../../../../subsystems/workspace/reference/plugins/ranking-visibility/guest.wasm"
    );
    let contributed = WorkerRequest::RankingContribute {
        guest: ranking.to_vec(),
        hits: vec![
            WorkerRankingHit {
                file: "src/lib.rs".to_owned(),
                line: 1,
                column: 1,
                excerpt: "pub fn x() {}".to_owned(),
                kind_class: "declaration".to_owned(),
                file_class: "source".to_owned(),
            },
            WorkerRankingHit {
                file: "src/lib.rs".to_owned(),
                line: 2,
                column: 1,
                excerpt: "x();".to_owned(),
                kind_class: "call".to_owned(),
                file_class: "source".to_owned(),
            },
        ],
        capabilities: Vec::new(),
        policy,
    }
    .execute()
    .expect("ranking guest outcome is typed data");
    let WorkerResponse::RankingContributed {
        end, contributions, ..
    } = contributed
    else {
        panic!("ranking dispatch returns the ranking variant");
    };
    assert_eq!(end, WorkerGuestEnd::Ok);
    assert_eq!(contributions.len(), 2);
    assert_eq!(contributions[0][0].name, "exported");
    assert_eq!(contributions[0][0].value, 1.0);
    assert!(contributions[1].is_empty());

    let runner =
        include_bytes!("../../../../subsystems/workspace/reference/plugins/runner-go/guest.wasm");
    let parsed = WorkerRequest::RunnerParse {
        guest: runner.to_vec(),
        command: "go test ./...".to_owned(),
        exit_code: 1,
        stdout: "--- FAIL: TestAdd (0.00s)\n    math_test.go:12: expected 4, got 5\nFAIL\nFAIL\texample.com/m\t0.004s\n".to_owned(),
        stderr: String::new(),
        capabilities: Vec::new(),
        policy,
    }
    .execute()
    .expect("runner guest outcome is typed data");
    let WorkerResponse::RunnerParsed {
        end,
        claimed,
        report,
        ..
    } = parsed
    else {
        panic!("runner dispatch returns the runner variant");
    };
    assert_eq!(end, WorkerGuestEnd::Ok);
    assert!(claimed);
    let report = report.expect("structured report");
    assert_eq!(report.failures[0].test_name, "TestAdd");
    assert_eq!(report.failures[0].file.as_deref(), Some("math_test.go"));
    assert_eq!(report.failures[0].assertion, "expected 4, got 5");
    assert_eq!(
        report.summary.as_deref(),
        Some("FAIL\texample.com/m\t0.004s")
    );
    let wire = serde_json::to_vec(&WorkerResponse::RunnerParsed {
        end: WorkerGuestEnd::Ok,
        fuel_used: 1,
        claimed,
        report: Some(report.clone()),
    })
    .expect("serializes");
    assert!(
        matches!(serde_json::from_slice::<WorkerResponse>(&wire), Ok(WorkerResponse::RunnerParsed { report: Some(back), .. }) if back == report)
    );
}
