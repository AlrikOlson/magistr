use super::*;
use crate::cost::cost_entry::CostEntry;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn sample_entry(id: &str, project: &str, cost: f64) -> CostEntry {
    CostEntry {
        session_id: id.into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: cost,
        tokens: TokenBreakdown::default(),
        turns: 3,
        duration_ms: 15000,
        actor: None,
        model: None,
    }
}

#[test]
fn filter_by_project_matches_substring() {
    let entries = vec![
        sample_entry("s1", "my-project", 1.0),
        sample_entry("s2", "other-thing", 2.0),
        sample_entry("s3", "My-Project-V2", 3.0),
    ];

    let filtered = filter_by_project(&entries, "my-project");
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].session_id, "s1");
    assert_eq!(filtered[1].session_id, "s3");
}
