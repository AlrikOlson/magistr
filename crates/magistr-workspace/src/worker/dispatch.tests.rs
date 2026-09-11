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
