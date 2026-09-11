use super::*;
use crate::response::ResponseAdapter;

#[test]
fn the_row_wears_the_retained_field_names_and_accounting() {
    let root = Path::new("/tmp/workspace");
    let detection = NetworkDetection::detect(root, Some("0"));
    let response = StatusRow::new(root, &detection, "wal").respond(Vec::new());
    let sealed = ResponseAdapter::seal(&response).expect("sealed");
    let envelope = sealed.structured();
    assert_eq!(envelope["v"], 1);
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["freshness"], "warm");
    assert_eq!(
        envelope["budget"],
        serde_json::json!({"requested": 1, "used": 1})
    );
    let row = &envelope["results"][0];
    assert_eq!(row["root"], "/tmp/workspace");
    assert_eq!(row["journalMode"], "WAL");
    assert_eq!(row["appliedJournalMode"], "wal");
    assert_eq!(row["networkFs"], false);
    assert_eq!(row["detectionMethod"], "env-override");
    assert_eq!(row["envelope"], serde_json::json!({"min": 1, "latest": 1}));
    assert!(envelope.get("notes").is_none(), "no notes on a healthy row");
}

#[test]
fn a_network_workspace_reports_truncate_and_the_notes_ride_the_envelope() {
    let root = Path::new("/mnt/share/workspace");
    let detection = NetworkDetection::detect(root, Some("1"));
    let response = StatusRow::new(root, &detection, "truncate").respond(vec!["degraded".into()]);
    let WorkspaceResponse::Success { results, notes, .. } = response else {
        panic!("status is a success envelope");
    };
    assert_eq!(results.rows()[0].journal_mode, "TRUNCATE");
    assert!(results.rows()[0].network_fs);
    assert_eq!(notes, Some(vec!["degraded".to_owned()]));
}

#[test]
fn two_observations_of_one_workspace_are_byte_identical() {
    let root = Path::new("/tmp/workspace");
    let first = StatusRow::new(root, &NetworkDetection::detect(root, Some("0")), "wal");
    let second = StatusRow::new(root, &NetworkDetection::detect(root, Some("0")), "wal");
    assert_eq!(
        serde_json::to_string(&first).expect("json"),
        serde_json::to_string(&second).expect("json")
    );
}
