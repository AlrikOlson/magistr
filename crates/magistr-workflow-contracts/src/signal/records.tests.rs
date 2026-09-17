use super::*;

#[test]
fn status_transition_table_is_forward_plus_dismiss() {
    use SignalStatus::*;
    // Forward pipeline, one step at a time.
    assert!(New.allows(Triaged));
    assert!(Triaged.allows(Researched));
    assert!(Researched.allows(Surfaced));
    assert!(Surfaced.allows(Promoted));
    // Dismiss from any non-terminal state.
    assert!(New.allows(Dismissed));
    assert!(Surfaced.allows(Dismissed));
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
        serde_json::to_value(SignalKind::Feedback).unwrap(),
        serde_json::json!("feedback")
    );
    assert_eq!(
        serde_json::to_value(SignalStatus::Researched).unwrap(),
        serde_json::json!("researched")
    );
}

fn signal(id: &str, status: SignalStatus) -> Signal {
    Signal {
        id: id.into(),
        kind: SignalKind::Question,
        from: "t@example.com".into(),
        body: "body".into(),
        content: None,
        created: "2026-06-09T10:00:00+00:00".into(),
        status,
        enrichment: Vec::new(),
        cross_refs: Vec::new(),
        surfaced_at: None,
        snooze_until: None,
        project_id: None,
    }
}

fn store(signals: Vec<Signal>) -> Signals {
    Signals {
        project_id: "p".into(),
        signals,
    }
}

#[test]
fn merge_unions_disk_only_signals() {
    let memory = store(vec![signal("mine", SignalStatus::New)]);
    let disk = store(vec![signal("theirs", SignalStatus::New)]);
    let merged = merge_signal_stores(&memory, disk);
    let ids: Vec<&str> = merged.signals.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["mine", "theirs"]);
}

#[test]
fn merge_conflict_keeps_the_furthest_lifecycle_progress() {
    // Disk further along → disk wins.
    let memory = store(vec![signal("x", SignalStatus::New)]);
    let disk = store(vec![signal("x", SignalStatus::Researched)]);
    let merged = merge_signal_stores(&memory, disk);
    assert_eq!(merged.signals.len(), 1);
    assert_eq!(merged.signals[0].status, SignalStatus::Researched);

    // Memory further along → memory sticks.
    let memory = store(vec![signal("x", SignalStatus::Surfaced)]);
    let disk = store(vec![signal("x", SignalStatus::Triaged)]);
    assert_eq!(
        merge_signal_stores(&memory, disk).signals[0].status,
        SignalStatus::Surfaced
    );

    // Promoted outranks Dismissed (the promotion spawned a chunk).
    let memory = store(vec![signal("x", SignalStatus::Dismissed)]);
    let disk = store(vec![signal("x", SignalStatus::Promoted)]);
    assert_eq!(
        merge_signal_stores(&memory, disk).signals[0].status,
        SignalStatus::Promoted
    );
}

#[test]
fn merge_same_rank_prefers_more_enrichment_then_memory() {
    let enrichment = Enrichment {
        think_step: None,
        sources: vec![],
        summary: "deeper".into(),
        confidence: 0.8,
        at: "2026-06-09T10:00:00+00:00".into(),
    };
    // Same status, disk has the enrichment another process appended.
    let memory = store(vec![signal("x", SignalStatus::Triaged)]);
    let mut enriched = signal("x", SignalStatus::Triaged);
    enriched.enrichment.push(enrichment);
    let disk = store(vec![enriched]);
    assert_eq!(
        merge_signal_stores(&memory, disk).signals[0]
            .enrichment
            .len(),
        1
    );

    // Full tie → memory wins.
    let mut mine = signal("x", SignalStatus::New);
    mine.body = "memory copy".into();
    let memory = store(vec![mine]);
    let disk = store(vec![signal("x", SignalStatus::New)]);
    assert_eq!(
        merge_signal_stores(&memory, disk).signals[0].body,
        "memory copy"
    );
}
