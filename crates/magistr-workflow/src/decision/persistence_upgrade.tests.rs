use super::*;
use crate::infra::PersistenceConfig;
use std::path::PathBuf;
/// A per-log history saved before the project document: older keys and the
/// reference words from before the run words.
const OLD: &[u8] = include_bytes!("fixtures/v1.json");
/// A real project document saved after the plan words and before the
/// decision words: steps, branches and `think:` references.
const BEFORE_DECISION_WORDS: &[u8] = include_bytes!("fixtures/v4.json");

fn owner() -> Persistence {
    owner_of("saved-project")
}

fn owner_of(project: &str) -> Persistence {
    Persistence::for_project(
        &PersistenceConfig {
            enabled: false,
            data_dir: PathBuf::new(),
        },
        project,
    )
}

#[test]
fn older_default_and_named_histories_are_consolidated_without_losing_notes() {
    let mut named: Value = serde_json::from_slice(OLD).unwrap();
    named["history"]["session_id"] = json!("saved-project__review");
    named["history"]["steps"][0]["step_number"] = json!(8);
    named["history"]["steps"][0]
        .as_object_mut()
        .unwrap()
        .remove("record_id");
    let documents = BTreeMap::from([
        ("saved-project".into(), OLD.to_vec()),
        (
            "saved-project__review".into(),
            serde_json::to_vec(&named).unwrap(),
        ),
    ]);
    let bytes = owner().upgrade_documents(&documents).unwrap();
    let converted: PersistedProject = serde_json::from_slice(&bytes).unwrap();
    owner().validate(&converted).unwrap();
    assert_eq!(converted.logs.len(), 2);
    let default = &converted.logs["saved-project"];
    assert_eq!(default.notes[0].note, 7);
    assert_eq!(default.notes[0].record_id.as_deref(), Some("retained-step"));
    // Run references in the older history take the run words.
    assert_eq!(default.notes[0].run_ref.as_deref(), Some("step:one"));
    assert_eq!(default.threads.as_deref(), Some(&[][..]));
    assert_eq!(
        default.metadata.as_ref().and_then(|m| m.threads_created),
        Some(0)
    );
    let review = &converted.logs["saved-project__review"];
    assert_eq!(review.notes[0].note, 8);
    assert_eq!(review.log_id.as_deref(), Some("saved-project__review"));
    assert!(review.notes[0].record_id.is_some());
    assert_eq!(owner().upgrade_documents(&documents).unwrap(), bytes);
    let current = BTreeMap::from([("saved-project".into(), bytes.clone())]);
    assert_eq!(owner().upgrade_documents(&current).unwrap(), bytes);
}

#[test]
fn a_document_saved_before_the_decision_words_takes_them_once() {
    let project = "magistr-5a5bc6";
    let bytes = Persistence::upgrade_document(BEFORE_DECISION_WORDS, project).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    for old in [
        "\"histories\"",
        "\"steps\"",
        "\"branches\"",
        "\"step_number\"",
        "\"branch_from\"",
        "\"from_step\"",
        "\"execution_ref\"",
        "\"session_id\"",
        "\"branches_created\"",
        "\"think:",
        "\"active\"",
    ] {
        assert!(!text.contains(old), "{old} survived the upgrade");
    }
    let converted: PersistedProject = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(converted.schema_version, SCHEMA_VERSION);
    owner_of(project).validate(&converted).unwrap();
    let log = &converted.logs[project];
    let numbers: Vec<u32> = log.notes.iter().map(|note| note.note).collect();
    assert_eq!(numbers, [1752, 1753, 1754, 1755]);
    assert_eq!(log.notes[0].run_ref.as_deref(), Some("step:ttt-opponent"));
    assert_eq!(
        log.notes[0].dependencies.as_deref(),
        Some(
            &[crate::decision::domain::DepEdge::Tagged {
                note: 1751,
                relation: Some("supports".into()),
            }][..]
        )
    );
    let threads = log.threads.as_deref().unwrap();
    let open = threads
        .iter()
        .find(|thread| thread.id == "branch-1789601738023-0001")
        .unwrap();
    assert_eq!(open.status, crate::decision::domain::ThreadStatus::Open);
    let merged = threads
        .iter()
        .find(|thread| thread.id == "branch-607f5db1-f3ad-449b-ac72-1cb45b759a9b")
        .unwrap();
    assert_eq!(merged.from_note, 1752);
    assert_eq!(merged.merged_into, Some(1754));
    assert_eq!(merged.notes[0].note, 1753);
    assert_eq!(
        Persistence::upgrade_document(&bytes, project).unwrap(),
        bytes,
        "a current document comes back unchanged"
    );
}

#[test]
fn readers_refuse_the_older_words_after_the_upgrade() {
    let old: Value = serde_json::from_slice(BEFORE_DECISION_WORDS).unwrap();
    let mut relabelled = old.clone();
    relabelled["schema_version"] = json!(SCHEMA_VERSION);
    assert!(serde_json::from_value::<PersistedProject>(relabelled).is_err());
}

#[test]
fn inconsistent_histories_are_refused_instead_of_silently_renumbered() {
    for change in ["version", "project", "session", "number", "identity"] {
        let mut bad: Value = serde_json::from_slice(OLD).unwrap();
        match change {
            "version" => bad["schema_version"] = json!(99),
            "project" => bad["history"]["metadata"]["project_id"] = json!("another"),
            "session" => bad["history"]["session_id"] = json!("another__review"),
            "number" => {
                let step = bad["history"]["steps"][0].clone();
                bad["history"]["steps"].as_array_mut().unwrap().push(step);
            }
            _ => bad["history"]["steps"][0]["record_id"] = json!(""),
        }
        let documents =
            BTreeMap::from([("saved-project".into(), serde_json::to_vec(&bad).unwrap())]);
        assert!(owner().upgrade_documents(&documents).is_err(), "{change}");
    }
}
