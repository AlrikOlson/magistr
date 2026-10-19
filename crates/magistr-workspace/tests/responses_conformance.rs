//! The same semantic fixtures execute through the native adapter against the current operation contract.
use magistr_workspace::contracts::{
    Budget, EnvelopeVersion, ErrorKind, Freshness, PortVersion, Recovery, ResultSet,
    TruncationReason, WorkspaceError, WorkspaceResponse,
};
use magistr_workspace::response::{Redactor, ResponseAdapter};
use serde_json::{Value, json};

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../fixtures/contracts/responses.json"))
        .expect("response fixtures")
}

#[test]
fn policy_failures_preserve_recovery_and_message() {
    let error = WorkspaceError::new(ErrorKind::PathOutsideWorkspace, "outside");
    assert_eq!(error.code(), "PATH_OUTSIDE_WORKSPACE");
    assert_eq!(error.recovery(), Recovery::Abort);
    assert_eq!(error.to_string(), "PATH_OUTSIDE_WORKSPACE: outside");
}

#[test]
fn named_and_entropy_redaction_matches_current_contract_including_unicode_boundaries() {
    for fixture in fixtures()["redaction"].as_array().expect("redactions") {
        let input = fixture["input"].as_str().expect("input");
        let redacted = Redactor::redact(input);
        assert_eq!(
            json!({"text": redacted.text, "kinds": redacted.kinds}),
            fixture["expected"],
            "input {input:?}"
        );
    }
    assert_eq!(Redactor::entropy(""), 0.0);
    assert_eq!(Redactor::entropy("aaaaaaaa"), 0.0);
    assert_eq!(Redactor::entropy("abcdefghijklmnop"), 4.0);
}

#[test]
fn version_negotiation_is_stateless_and_matches_current_contract() {
    let fixtures = fixtures();
    let cases = fixtures["negotiation"].as_array().expect("negotiations");
    for fixture in cases.iter().chain(cases.iter().rev()) {
        let actual = match ResponseAdapter::negotiate_meta(fixture["meta"].as_object()) {
            Ok(version) => json!({"ok":true,"v":version.get()}),
            Err(error) => {
                let response = WorkspaceResponse::<Value>::Failure {
                    version: EnvelopeVersion::LATEST,
                    error,
                    notes: None,
                };
                json!({"ok": false, "envelope": ResponseAdapter::seal(&response).expect("seal").structured()})
            }
        };
        assert_eq!(actual, fixture["expected"], "meta {}", fixture["meta"]);
    }
    for zero in [0.0, -0.0] {
        let meta = json!({"magistr/maxEnvelopeVersion": zero});
        let error = ResponseAdapter::negotiate_meta(meta.as_object()).expect_err("zero");
        assert!(error.message.ends_with("got 0"), "{}", error.message);
    }
    assert_eq!(
        EnvelopeVersion::negotiate(None).expect("latest"),
        EnvelopeVersion::LATEST
    );
    assert_eq!(
        EnvelopeVersion::negotiate(Some(0))
            .expect_err("zero")
            .recovery(),
        Recovery::Abort
    );
}

#[test]
fn all_taxonomy_codes_preserve_required_payloads_and_recovery() {
    let fixtures = fixtures();
    let cases = fixtures["errors"].as_array().expect("errors");
    let taxonomy: Value = serde_json::from_str(include_str!("../schemas/error-taxonomy.json"))
        .expect("normative taxonomy");
    assert_eq!(
        cases.len(),
        taxonomy["codes"].as_array().expect("codes").len()
    );
    for (fixture, entry) in cases
        .iter()
        .zip(taxonomy["codes"].as_array().expect("codes"))
    {
        let input = &fixture["input"];
        let error = WorkspaceError::new(
            error_kind(input),
            input["message"].as_str().expect("message"),
        );
        assert_eq!(error.code(), entry["code"].as_str().expect("code"));
        assert_eq!(
            error.recovery().as_str(),
            entry["recovery"].as_str().expect("recovery")
        );
        let response = WorkspaceResponse::<Value>::Failure {
            version: EnvelopeVersion::LATEST,
            error,
            notes: notes(input),
        };
        assert_sealed(&response, &fixture["expected"]);
    }
}

fn error_kind(input: &Value) -> ErrorKind {
    let text = |field: &str| input[field].as_str().expect("payload string").to_owned();
    match input["code"].as_str().expect("code") {
        "PATH_OUTSIDE_WORKSPACE" => ErrorKind::PathOutsideWorkspace,
        "PROTECTED_PATH" => ErrorKind::ProtectedPath,
        "AMBIGUOUS_SYMBOL" => ErrorKind::AmbiguousSymbol {
            candidates: serde_json::from_value(input["candidates"].clone()).expect("candidates"),
        },
        "SYMBOL_NOT_FOUND" => ErrorKind::SymbolNotFound,
        "FILE_NOT_FOUND" => ErrorKind::FileNotFound,
        "PARSE_REJECTED" => ErrorKind::ParseRejected {
            parser_message: text("parserMessage"),
            rejected_diff: text("rejectedDiff"),
        },
        "STALE_ANCHOR" => ErrorKind::StaleAnchor {
            content_hash: text("contentHash"),
        },
        "PATTERN_INVALID" => ErrorKind::PatternInvalid,
        "BUDGET_TOO_SMALL" => ErrorKind::BudgetTooSmall,
        "COMMAND_NOT_DECLARED" => ErrorKind::CommandNotDeclared,
        "RUN_REDIRECTED" => ErrorKind::RunRedirected,
        "SANDBOX_FUEL_EXCEEDED" => ErrorKind::SandboxFuelExceeded,
        "SANDBOX_CAP_DENIED" => ErrorKind::SandboxCapDenied,
        "SANDBOX_MEMORY_EXCEEDED" => ErrorKind::SandboxMemoryExceeded,
        "INDEX_UNAVAILABLE" => ErrorKind::IndexUnavailable,
        "CONFIRM_REQUIRED" => ErrorKind::ConfirmRequired {
            confirm_token: text("confirmToken"),
        },
        "CONFIRM_MISMATCH" => ErrorKind::ConfirmMismatch,
        "UNSUPPORTED_VERSION" => ErrorKind::UnsupportedVersion,
        "ENGINE_UNAVAILABLE" => ErrorKind::EngineUnavailable,
        "ENGINE_BUSY" => ErrorKind::EngineBusy {
            backoff_ms: input["backoffMs"].as_u64().expect("backoff"),
        },
        "INPUT_QUARANTINED" => ErrorKind::InputQuarantined,
        "PLUGIN_CAP_DENIED" => ErrorKind::PluginCapDenied,
        "PLUGIN_CRASHED" => ErrorKind::PluginCrashed {
            plugin: text("plugin"),
        },
        "CONFIG_POLICY_DENIED" => ErrorKind::ConfigPolicyDenied,
        code => panic!("unmapped fixture code {code}"),
    }
}

fn notes(input: &Value) -> Option<Vec<String>> {
    input
        .get("notes")
        .map(|notes| serde_json::from_value(notes.clone()).expect("notes"))
}

fn assert_sealed(response: &WorkspaceResponse<Value>, expected: &Value) {
    let sealed = ResponseAdapter::seal(response).expect("seal");
    assert_eq!(sealed.structured(), expected);
    assert_eq!(
        serde_json::from_str::<Value>(sealed.json()).expect("serialized twin"),
        *sealed.structured()
    );
    assert_eq!(
        ResponseAdapter::seal(response).expect("repeat").json(),
        sealed.json()
    );
}

#[test]
fn success_shapes_cuts_and_optional_notes_match_current_contract() {
    for fixture in fixtures()["success"].as_array().expect("successes") {
        let input = &fixture["input"];
        let reason = match input["cut"]["reason"].as_str().expect("reason") {
            "budget" => TruncationReason::Budget,
            "ceiling" => TruncationReason::Ceiling,
            "timeout" => TruncationReason::Timeout,
            other => panic!("unknown reason {other}"),
        };
        let results = ResultSet::truncate(
            input["rows"].as_array().expect("rows").clone(),
            input["keep"].as_u64().expect("keep") as usize,
            reason,
            serde_json::from_value(input["cut"]["narrow"].clone()).expect("hints"),
            input["cut"]["cursor"].as_str().expect("cursor").into(),
        )
        .expect("valid cut");
        let response = WorkspaceResponse::Success {
            version: EnvelopeVersion::LATEST,
            freshness: Freshness::Warm,
            budget: Budget {
                requested: 4,
                used: 2,
            },
            results,
            notes: notes(input),
        };
        assert_sealed(&response, &fixture["expected"]);
    }
}

#[test]
fn a_cut_requires_a_complete_manifest_but_complete_rows_need_no_cursor() {
    for length in 0..8 {
        for keep in 0..10 {
            let rows: Vec<_> = (0..length).collect();
            let result =
                ResultSet::truncate(rows, keep, TruncationReason::Budget, vec![], "next".into())
                    .expect("cursor");
            assert_eq!(result.rows().len(), keep.min(length));
            assert_eq!(
                result.truncation().map(|cut| cut.dropped()),
                (keep < length).then_some(length.saturating_sub(keep))
            );
        }
    }
    let rejected = ResultSet::truncate(vec![1], 0, TruncationReason::Budget, vec![], String::new());
    assert_eq!(
        rejected.expect_err("missing cursor").code(),
        "BUDGET_TOO_SMALL"
    );
    let complete =
        ResultSet::truncate(vec![1], 8, TruncationReason::Timeout, vec![], String::new())
            .expect("no cut");
    assert!(complete.truncation().is_none());
}

#[test]
fn signature_versions_match_the_current_registry() {
    let actual: serde_json::Map<String, Value> = PortVersion::ALL
        .into_iter()
        .map(|port| (port.name().into(), json!(port.signature_version())))
        .collect();
    assert_eq!(Value::Object(actual), fixtures()["ports"]);
}
