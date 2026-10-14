use super::*;
use magistr_workflow_contracts::{ReadErrorKind, WriteErrorKind};
use serde::Deserialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct State {
    schema: u32,
    owner: String,
    records: Vec<String>,
}

fn empty() -> State {
    State {
        schema: 1,
        owner: "project".into(),
        records: Vec::new(),
    }
}

fn validate(state: &State) -> Result<(), ReadError> {
    if state.schema != 1 {
        return Err(ReadError::new(
            ReadErrorKind::Schema,
            "This saved work cannot be opened.",
        ));
    }
    if state.owner != "project" {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "This saved work belongs to another project.",
        ));
    }
    Ok(())
}

fn append(
    path: &Path,
    text: &str,
) -> Result<Committed<State, usize>, TransactionError<&'static str>> {
    transact(
        TransactionTarget::File {
            path,
            lock_path: &path.with_extension("json.lock"),
        },
        empty,
        validate,
        |state| {
            state.records.push(text.into());
            Ok(state.records.len())
        },
    )
}

#[test]
fn invalid_storage_never_becomes_an_empty_store_or_acknowledgement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    for (bytes, kind) in [
        ("not json", ReadErrorKind::Decode),
        (
            r#"{"schema":2,"owner":"project","records":["keep"]}"#,
            ReadErrorKind::Schema,
        ),
        (
            r#"{"schema":1,"owner":"another","records":["keep"]}"#,
            ReadErrorKind::Identity,
        ),
    ] {
        fs::write(&path, bytes).unwrap();
        let error = append(&path, "new").unwrap_err();
        assert!(
            matches!(error, TransactionError::Store(WriteError { kind: WriteErrorKind::Read(actual), .. }) if actual == kind)
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(matches!(
        append(&path, "new"),
        Err(TransactionError::Store(_))
    ));
    assert!(path.is_dir());
}

#[test]
fn rejected_mutation_discards_its_candidate_and_retry_reads_fresh_saved_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    append(&path, "kept").unwrap();
    let before = fs::read(&path).unwrap();
    let result: Result<Committed<State, ()>, _> = transact(
        TransactionTarget::File {
            path: &path,
            lock_path: &path.with_extension("json.lock"),
        },
        empty,
        validate,
        |state| {
            state.records.clear();
            Err("invalid change")
        },
    );
    assert!(matches!(
        result,
        Err(TransactionError::Rejected("invalid change"))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    let next = append(&path, "retry").unwrap();
    assert_eq!(next.state.records, ["kept", "retry"]);
    assert_eq!(next.value, 2);
    assert_eq!(read_json::<State>(&path).unwrap().source, next.source);
}

#[test]
fn memory_only_commit_is_explicit_and_leaves_the_input_unchanged() {
    let original = empty();
    let committed = transact(
        TransactionTarget::Memory(&original),
        empty,
        validate,
        |state| {
            state.records.push("new".into());
            Ok::<_, ()>(1)
        },
    )
    .unwrap();
    assert_eq!(committed.source, ReadSource::Memory);
    assert!(original.records.is_empty());
    assert_eq!(committed.state.records, ["new"]);
}

#[test]
fn concurrent_owners_preserve_every_record_and_return_their_actual_commit() {
    use std::sync::{Arc, Barrier};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    let barrier = Arc::new(Barrier::new(8));
    let joins: Vec<_> = (0..8)
        .map(|n| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let record = format!("record-{n}");
                let committed = append(&path, &record).unwrap();
                assert_eq!(committed.state.records.last(), Some(&record));
                committed.value
            })
        })
        .collect();
    let mut counts: Vec<_> = joins.into_iter().map(|join| join.join().unwrap()).collect();
    counts.sort_unstable();
    assert_eq!(counts, (1..=8).collect::<Vec<_>>());
    let stored = read_json::<State>(&path).unwrap().value.unwrap();
    assert_eq!(stored.records.len(), 8);
    let unique: std::collections::BTreeSet<_> = stored.records.iter().collect();
    assert_eq!(unique.len(), 8);
    assert!(path.with_extension("json.lock").is_file());
}

#[test]
fn missing_file_uses_the_canonical_empty_state_without_resurrecting_old_records() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    append(&path, "discarded").unwrap();
    fs::remove_file(&path).unwrap();
    let next = append(&path, "new").unwrap();
    assert_eq!(next.state.records, ["new"]);
    assert_eq!(next.value, 1);
}
