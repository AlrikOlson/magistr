use super::*;
use crate::cost::append_entry::append_entry;
use crate::cost::cost_entry::CostEntry;
use crate::cost::costs_path::costs_path;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;
use tempfile::TempDir;

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
fn load_missing_file_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");
    let entries = load_entries(&path).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn load_empty_file_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");
    std::fs::write(&path, "").unwrap();
    let entries = load_entries(&path).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn append_and_load_round_trip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");

    let e1 = sample_entry("s1", "project-a", 1.50);
    let e2 = sample_entry("s2", "project-b", 2.25);

    append_entry(&path, &e1).unwrap();
    append_entry(&path, &e2).unwrap();

    let loaded = load_entries(&path).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].session_id, "s1");
    assert_eq!(loaded[1].session_id, "s2");
    assert_eq!(loaded[0].cost_usd, Some(1.50));
    assert_eq!(loaded[1].cost_usd, Some(2.25));
}

#[test]
fn append_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("deep/nested/costs.json");

    let entry = sample_entry("s1", "proj", 0.50);
    append_entry(&path, &entry).unwrap();

    assert!(path.exists());
    let loaded = load_entries(&path).unwrap();
    assert_eq!(loaded.len(), 1);
}

#[test]
fn the_ledger_lives_in_the_captured_data_dir() {
    let inputs = crate::config::inputs::ProjectInputs::from_lookup("/proj", |key| {
        (key == "XDG_DATA_HOME").then(|| "/xdg-data".to_owned())
    });
    assert_eq!(
        costs_path(&inputs),
        Some(std::path::PathBuf::from("/xdg-data/magistr/costs.json"))
    );
    let nowhere = crate::config::inputs::ProjectInputs::from_lookup("/proj", |_| None);
    assert_eq!(costs_path(&nowhere), None, "no home, no ledger");
}

#[test]
fn invalid_json_returns_error() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");
    std::fs::write(&path, "not valid json").unwrap();

    let result = load_entries(&path);
    assert!(result.is_err());
}
