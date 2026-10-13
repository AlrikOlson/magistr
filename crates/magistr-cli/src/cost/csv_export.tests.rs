use super::*;
use crate::cost::cost_entry::CostEntry;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn sample_entry(id: &str, project: &str, cost: f64) -> CostEntry {
    CostEntry {
        session_id: id.into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: Some(cost),
        tokens: TokenBreakdown::default(),
        turns: 3,
        duration_ms: 15000,
        actor: None,
        model: None,
    }
}

#[test]
fn csv_empty_entries() {
    let csv = to_csv(&[]);
    assert!(csv.starts_with("session_id,"));
    // Only the header line
    assert_eq!(csv.lines().count(), 1);
}

#[test]
fn csv_single_entry() {
    let entries = vec![sample_entry("s1", "my-project", 1.5)];
    let csv = to_csv(&entries);
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[1].starts_with("s1,my-project,"));
    assert!(lines[1].contains("1.500000"));
}

#[test]
fn csv_escapes_commas_in_project() {
    let mut entry = sample_entry("s1", "project,with,commas", 1.0);
    entry.project = "project,with,commas".into();
    let csv = to_csv(&[entry]);
    assert!(csv.contains("\"project,with,commas\""));
}

#[test]
fn csv_escape_plain_string() {
    assert_eq!(csv_escape("hello"), "hello");
}

#[test]
fn csv_escape_with_comma() {
    assert_eq!(csv_escape("a,b"), "\"a,b\"");
}

#[test]
fn csv_escape_with_quotes() {
    assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
}

#[test]
fn the_header_carries_the_attribution_columns() {
    let csv = to_csv(&[]);
    let header = csv.lines().next().expect("a header row");
    assert_eq!(
        header,
        "session_id,project,actor,model,timestamp,cost_usd,turns,duration_ms,input_tokens,output_tokens,cache_read_tokens,cache_write_tokens"
    );
}

#[test]
fn an_attributed_entry_fills_the_actor_and_model_columns() {
    let mut entry = sample_entry("s1", "magistr", 1.0);
    entry.actor = Some("ada@example.com".into());
    entry.model = Some("claude-opus-5".into());
    let csv = to_csv(&[entry]);
    let row = csv.lines().nth(1).expect("a data row");
    assert!(
        row.starts_with("s1,magistr,ada@example.com,claude-opus-5,"),
        "{row}"
    );
}

#[test]
fn an_unattributed_entry_leaves_the_columns_empty() {
    let csv = to_csv(&[sample_entry("s1", "magistr", 1.0)]);
    let row = csv.lines().nth(1).expect("a data row");
    assert!(row.starts_with("s1,magistr,,,"), "{row}");
}
