use super::*;

#[test]
fn status_transition_table_is_forward_plus_dismiss() {
    use RequestStatus::*;
    // Forward pipeline, one step at a time.
    assert!(New.allows(Triaged));
    assert!(Triaged.allows(Researched));
    assert!(Researched.allows(Raised));
    assert!(Raised.allows(Promoted));
    // Dismiss from any non-terminal state.
    assert!(New.allows(Dismissed));
    assert!(Raised.allows(Dismissed));
    // Backward + skip-ahead are illegal.
    assert!(!Researched.allows(New));
    assert!(!New.allows(Researched));
    assert!(!New.allows(Promoted));
    // Terminal states allow nothing.
    assert!(!Promoted.allows(Dismissed));
    assert!(!Dismissed.allows(New));
}

#[test]
fn kind_and_status_serialize_snake_case() {
    assert_eq!(
        serde_json::to_value(RequestKind::Feedback).unwrap(),
        serde_json::json!("feedback")
    );
    assert_eq!(
        serde_json::to_value(RequestStatus::Researched).unwrap(),
        serde_json::json!("researched")
    );
}

fn request(id: &str, status: RequestStatus) -> Request {
    Request {
        id: id.into(),
        kind: RequestKind::Question,
        from: "t@example.com".into(),
        body: "body".into(),
        content: None,
        created: "2026-06-09T10:00:00+00:00".into(),
        status,
        enrichment: Vec::new(),
        refs: Vec::new(),
        raised_at: None,
        snooze_until: None,
        project_id: None,
    }
}

fn store(requests: Vec<Request>) -> Requests {
    Requests {
        schema_version: REQUESTS_SCHEMA_VERSION,
        project_id: "p".into(),
        requests,
    }
}

#[test]
fn merge_unions_disk_only_requests() {
    let memory = store(vec![request("mine", RequestStatus::New)]);
    let disk = store(vec![request("theirs", RequestStatus::New)]);
    let merged = merge_request_stores(&memory, disk);
    let ids: Vec<&str> = merged.requests.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["mine", "theirs"]);
}

#[test]
fn merge_conflict_keeps_the_furthest_lifecycle_progress() {
    // Disk further along → disk wins.
    let memory = store(vec![request("x", RequestStatus::New)]);
    let disk = store(vec![request("x", RequestStatus::Researched)]);
    let merged = merge_request_stores(&memory, disk);
    assert_eq!(merged.requests.len(), 1);
    assert_eq!(merged.requests[0].status, RequestStatus::Researched);

    // Memory further along → memory sticks.
    let memory = store(vec![request("x", RequestStatus::Raised)]);
    let disk = store(vec![request("x", RequestStatus::Triaged)]);
    assert_eq!(
        merge_request_stores(&memory, disk).requests[0].status,
        RequestStatus::Raised
    );

    // Promoted outranks Dismissed (the promotion spawned a task).
    let memory = store(vec![request("x", RequestStatus::Dismissed)]);
    let disk = store(vec![request("x", RequestStatus::Promoted)]);
    assert_eq!(
        merge_request_stores(&memory, disk).requests[0].status,
        RequestStatus::Promoted
    );
}

#[test]
fn merge_same_rank_prefers_more_enrichment_then_memory() {
    let enrichment = Enrichment {
        note: None,
        sources: vec![],
        summary: "deeper".into(),
        confidence: 0.8,
        at: "2026-06-09T10:00:00+00:00".into(),
    };
    // Same status, disk has the enrichment another process appended.
    let memory = store(vec![request("x", RequestStatus::Triaged)]);
    let mut enriched = request("x", RequestStatus::Triaged);
    enriched.enrichment.push(enrichment);
    let disk = store(vec![enriched]);
    assert_eq!(
        merge_request_stores(&memory, disk).requests[0]
            .enrichment
            .len(),
        1
    );

    // Full tie → memory wins.
    let mut mine = request("x", RequestStatus::New);
    mine.body = "memory copy".into();
    let memory = store(vec![mine]);
    let disk = store(vec![request("x", RequestStatus::New)]);
    assert_eq!(
        merge_request_stores(&memory, disk).requests[0].body,
        "memory copy"
    );
}
