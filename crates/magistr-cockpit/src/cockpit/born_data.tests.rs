use super::*;
use crate::harness::fixtures;

#[test]
fn the_toast_counts_the_chunks_and_workstreams_and_names_the_gates() {
    let mut born = BornData {
        name: "tidewatch".to_owned(),
        dir: "~/Code/tidewatch".to_owned(),
        chunks: vec![
            fixtures::chunk_from(serde_json::json!({"id": "a", "group": "core", "deps": []})),
            fixtures::chunk_from(serde_json::json!({"id": "b", "group": "core", "deps": ["a"]})),
            fixtures::chunk_from(serde_json::json!({"id": "c", "group": "ops"})),
        ],
        gates: Vec::new(),
        protected: Vec::new(),
    };
    assert_eq!(born.workstreams(), 2);
    assert_eq!(born.summary(), "3 chunks \u{b7} 2 workstreams");
    assert_eq!(born.first_ready(), Some("a"));
    born.gates.push("cargo test".to_owned());
    assert_eq!(
        born.summary(),
        "3 chunks \u{b7} 2 workstreams \u{b7} Gates configured"
    );
    assert_eq!(BornData::default().first_ready(), None);
}
