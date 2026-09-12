use serde_json::json;

use super::*;
use crate::harness::fixtures;

#[test]
fn a_record_reads_and_takes_the_best_confidence() {
    let signal = SignalRow::from_signal(&fixtures::signal_from(json!({
        "id": "s1",
        "kind": "bug",
        "from": "bob",
        "body": "it crashes",
        "status": "researched",
        "enrichment": [{"confidence": 0.3}, {"confidence": 0.9}, {"confidence": 0.5}],
    })));
    assert_eq!(signal.kind, "bug");
    assert_eq!(signal.confidence, Some(0.9));
    assert!(signal.is_open());
}

#[test]
fn terminal_statuses_leave_the_inbox() {
    let at = |status: &str| {
        SignalRow::from_signal(&fixtures::signal_from(json!({"id": "s", "status": status})))
            .is_open()
    };
    assert!(at("new") && at("researched") && at("surfaced"));
    assert!(!at("promoted") && !at("dismissed"));
}

#[test]
fn no_enrichment_means_no_confidence() {
    let signal = SignalRow::from_signal(&fixtures::signal_from(json!({"id": "s"})));
    assert_eq!(signal.confidence, None);
    assert_eq!(signal.findings, None);
    assert_eq!(signal.promoted_to, None);
    assert_eq!(signal.created_secs, None);
}

/// The Signals view's words come off the wire: the structured summary
/// titles the card (the body when there is none), the newest enrichment
/// with words is the findings, the `chunk:` cross-ref is the star.
#[test]
fn the_title_the_findings_and_the_star_are_read_off_the_record() {
    let signal = SignalRow::from_signal(&fixtures::signal_from(json!({
        "id": "s2",
        "body": "Affects all native windows.",
        "content": {"version": 1, "summary": "Retina window renders text at half size"},
        "status": "promoted",
        "enrichment": [
            {"summary": "first look", "confidence": 0.2},
            {"summary": "AccessKit 0.24 supports actions.", "confidence": 0.8},
            {"summary": "   ", "confidence": 0.1},
        ],
        "cross_refs": ["think:12", "chunk:e10c-session-migration", "chunk:other"],
    })));
    assert_eq!(signal.title(), "Retina window renders text at half size");
    assert_eq!(
        signal.findings.as_deref(),
        Some("AccessKit 0.24 supports actions."),
        "the newest enrichment with words"
    );
    assert_eq!(
        signal.promoted_to.as_deref(),
        Some("e10c-session-migration"),
        "the first chunk ref"
    );
    let bare = SignalRow::from_signal(&fixtures::signal_from(
        json!({"id": "s3", "body": "just words"}),
    ));
    assert_eq!(bare.title(), "just words", "no summary: the body titles");
}

/// The age reads in the coarsest unit that carries it, against the
/// clock the caller passes — never a clock of its own.
#[test]
fn the_age_is_the_coarsest_unit_against_the_given_now() {
    let signal = SignalRow::from_signal(&fixtures::signal_from(json!({
        "id": "s", "created": "2026-08-30T12:00:00Z",
    })));
    let created = signal.created_secs.expect("RFC 3339 parses");
    assert_eq!(created, 1_788_091_200, "2026-08-30T12:00:00Z");
    assert_eq!(
        signal.age_text(created + 2 * 86_400).as_deref(),
        Some("2 d")
    );
    assert_eq!(
        signal.age_text(created + 5 * 3_600 + 40).as_deref(),
        Some("5 h")
    );
    assert_eq!(
        signal.age_text(created + 12 * 60 + 59).as_deref(),
        Some("12 min")
    );
    assert_eq!(
        signal.age_text(created - 60).as_deref(),
        Some("0 s"),
        "no negative age"
    );
    let offset = SignalRow::from_signal(&fixtures::signal_from(
        json!({"id": "o", "created": "2026-08-30T14:00:00+02:00"}),
    ));
    assert_eq!(
        offset.created_secs,
        Some(created),
        "an offset stamp is the same instant"
    );
    let unstamped = SignalRow::from_signal(&fixtures::signal_from(
        json!({"id": "u", "created": "yesterday"}),
    ));
    assert_eq!(unstamped.age_text(created), None);
}
