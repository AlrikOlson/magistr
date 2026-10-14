use super::*;

fn refused() -> ReadError {
    ReadError::new(
        ReadErrorKind::Decode,
        "The saved work could not be read. Try again.",
    )
}

#[test]
fn failed_reads_retain_only_successful_data_until_a_real_recovery() {
    let mut state = ReadState::<Vec<&str>>::default();
    assert_eq!(state.current().unwrap_err().kind, ReadErrorKind::Unobserved);
    state.record(Err(refused()));
    assert!(matches!(state, ReadState::Unavailable(_)));
    assert!(state.last_successful().is_none());
    let observed = Observed {
        value: vec!["authored"],
        source: ReadSource::File(StoreStamp {
            inode: 7,
            modified: std::time::SystemTime::UNIX_EPOCH,
            length: 128,
        }),
    };
    state.record(Ok(observed.clone()));
    state.record(Err(refused()));
    state.record(Err(refused()));
    assert!(state.current().is_err());
    assert!(state.needs_retry());
    assert_eq!(state.last_successful(), Some(&observed));
    assert!(matches!(
        state.map(|items| items.len()),
        ReadState::Stale {
            last: Observed { value: 1, .. },
            ..
        }
    ));

    state.record(Ok(Observed {
        value: Vec::new(),
        source: ReadSource::Missing,
    }));
    assert!(!state.needs_retry());
    assert_eq!(state.current().unwrap().source, ReadSource::Missing);
    assert!(state.current().unwrap().value.is_empty());
}

#[test]
fn serialization_keeps_unavailable_distinct_from_known_empty_memory() {
    let mut state: ReadState<Vec<String>> = ReadState::Unavailable(refused());
    let encoded = serde_json::to_value(&state).unwrap();
    assert_eq!(encoded["state"], "unavailable");
    assert_eq!(
        serde_json::from_value::<ReadState<Vec<String>>>(encoded).unwrap(),
        state
    );
    state.record(Ok(Observed {
        value: Vec::new(),
        source: ReadSource::Memory,
    }));
    assert_eq!(serde_json::to_value(state).unwrap()["state"], "current");
}
