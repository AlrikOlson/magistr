use super::*;
use crate::cost::cost_entry::CostEntry;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

#[test]
fn filter_since_excludes_older() {
    let now = Utc::now();
    let old = now - chrono::Duration::hours(48);
    let recent = now - chrono::Duration::hours(1);

    let entries = vec![
        CostEntry {
            session_id: "old".into(),
            project: "p".into(),
            timestamp: old,
            cost_usd: Some(1.0),
            tokens: TokenBreakdown::default(),
            turns: 1,
            duration_ms: 1000,
            actor: None,
            model: None,
        },
        CostEntry {
            session_id: "recent".into(),
            project: "p".into(),
            timestamp: recent,
            cost_usd: Some(2.0),
            tokens: TokenBreakdown::default(),
            turns: 2,
            duration_ms: 2000,
            actor: None,
            model: None,
        },
    ];

    let cutoff = now - chrono::Duration::hours(24);
    let filtered = filter_since(&entries, cutoff);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].session_id, "recent");
}
